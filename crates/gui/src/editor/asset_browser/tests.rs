use super::*;

#[test]
fn audio_slider_tracks_playback_wraps_and_seeks_on_release() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("tone.ogg"),
        include_bytes!("../../../../runtime/tests/fixtures/tone.ogg"),
    )
    .unwrap();
    let manifest = ProjectManifest::new("Test", "Author");
    let mut cache = HashMap::new();
    let mut failures = HashMap::new();
    let mut resources = EditorResourceService::new();
    let ctx = egui::Context::default();
    let mut render = |position_ms, events| {
        let mut result = (0, false);
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(140.0, 100.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    result = AssetBrowserPanel::new(
                        &manifest,
                        Some(dir.path()),
                        &mut cache,
                        &mut failures,
                        &mut resources,
                    )
                    .with_audio_playback(Some(visual_novel_runtime::MusicPlayback {
                        path: "tone.ogg",
                        position: std::time::Duration::from_millis(position_ms),
                        looping: true,
                    }))
                    .render_audio_position(ui, "tone.ogg");
                });
            },
        );
        result
    };
    assert_eq!(render(50, vec![]), (50, false));
    assert_eq!(render(150, vec![]), (150, false));
    let wrapped = render(350, vec![]);
    assert!((99..=100).contains(&wrapped.0));
    let point = egui::pos2(110.0, 17.0);
    let _ = render(
        150,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let sought = render(
        155,
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(
        sought.1,
        "releasing the slider must issue a seek: {sought:?}"
    );
    assert!(
        sought.0 > 180,
        "must keep the chosen position until release: {sought:?}"
    );
}

#[test]
fn clipped_cards_skip_uploads_and_visible_cache_hits_protect_the_texture() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("tone.ogg"),
        include_bytes!("../../../../runtime/tests/fixtures/tone.ogg"),
    )
    .unwrap();
    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
        .save(dir.path().join("image.png"))
        .unwrap();
    let ctx = egui::Context::default();
    let manifest = ProjectManifest::new("Test", "Author");
    let mut cache = HashMap::new();
    let mut failures = HashMap::new();
    let mut resources = EditorResourceService::new();
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_clip_rect(egui::Rect::NOTHING);
            let response = AssetBrowserPanel::new(
                &manifest,
                Some(dir.path()),
                &mut cache,
                &mut failures,
                &mut resources,
            )
            .render_image_asset_card(ui, "bg", "image", Path::new("image.png"));
            assert!(
                response.rect.height() > 0.0,
                "clipping preserves card layout"
            );
            AssetBrowserPanel::new(
                &manifest,
                Some(dir.path()),
                &mut cache,
                &mut failures,
                &mut resources,
            )
            .render_audio_position(ui, "tone.ogg");
        });
    });
    assert!(
        cache.is_empty(),
        "clipped cards must not upload or pin textures"
    );
    assert_eq!(resources.metrics().misses, 0);
    assert!(failures.is_empty());

    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            AssetBrowserPanel::new(
                &manifest,
                Some(dir.path()),
                &mut cache,
                &mut failures,
                &mut resources,
            )
            .render_image_asset_card(ui, "bg", "image", Path::new("image.png"));
        });
    });
    assert_eq!(cache.len(), 1);
    let active_id = cache.values().next().unwrap().id();
    let inactive = ctx.load_texture(
        "inactive",
        egui::ColorImage::new([1, 1], egui::Color32::BLACK),
        egui::TextureOptions::LINEAR,
    );
    let inactive_id = inactive.id();
    for alias in 0..513 {
        cache.insert(format!("inactive-{alias}"), inactive.clone());
    }
    drop(inactive);
    let output = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            AssetBrowserPanel::new(
                &manifest,
                Some(dir.path()),
                &mut cache,
                &mut failures,
                &mut resources,
            )
            .render_image_asset_card(ui, "bg", "image", Path::new("image.png"));
        });
        resources.trim_texture_cache(&mut cache, ctx.frame_nr());
    });
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.values().next().unwrap().id(), active_id);
    assert!(output.textures_delta.free.contains(&inactive_id));
    assert!(!output.textures_delta.free.contains(&active_id));
    assert!(failures.is_empty());

    let mut panel = AssetBrowserPanel::new(
        &manifest,
        Some(dir.path()),
        &mut cache,
        &mut failures,
        &mut resources,
    );
    for missing in 0..600 {
        assert!(panel
            .thumbnail_texture(&ctx, &format!("missing-{missing}.png"))
            .is_none());
    }
    for missing in 0..600 {
        assert!(panel
            .audio_duration_secs(&format!("missing-{missing}.wav"))
            .is_none());
    }
    assert!(failures.len() <= crate::editor::image_asset_cache::MAX_ASSET_CACHE_ENTRIES);
}
