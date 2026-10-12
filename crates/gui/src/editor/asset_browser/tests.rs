use super::*;

#[test]
fn clipped_cards_skip_uploads_and_visible_cache_hits_protect_the_texture() {
    let dir = tempfile::tempdir().unwrap();
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
}
