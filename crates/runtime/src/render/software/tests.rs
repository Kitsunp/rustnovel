use super::*;
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use visual_novel_engine::runtime::{SceneFrame, UiState, UiView};

#[test]
fn overflowing_coordinates_do_not_touch_small_buffers() {
    let mut frame = [7; 4];
    blend_pixel(&mut frame, 65_536, 0, 16_384, [255; 4]);
    blend_pixel(&mut frame, u32::MAX, u32::MAX - 1, u32::MAX, [255; 4]);
    blend_pixel(&mut [], 1, 0, 0, [255; 4]);
    assert_eq!(frame, [7; 4]);
    blend_pixel(&mut frame, 1, 0, 0, [255; 4]);
    assert_eq!(frame, [255; 4]);
}

#[test]
fn image_cache_releases_inactive_entries_and_retries_failures_in_the_same_scene() {
    let mut assets = crate::MemoryAssetStore::default();
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&[255, 0, 0, 255], 1, 1, ColorType::Rgba8.into())
        .unwrap();
    let ui = UiState {
        view: UiView::Scene {
            description: "scene".into(),
        },
        pending_transition: None,
    };
    let mut drawer = BuiltinSoftwareDrawer::new();
    let mut pixels = [0; 4];
    let image_scene = |asset| SceneFrame {
        commands: vec![RenderCommand::Image {
            asset,
            rect: LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0,
            },
            fit: ImageFit::Stretch,
            z: 0,
        }],
        ..SceneFrame::default()
    };
    let mut draw = |drawer: &mut BuiltinSoftwareDrawer,
                    scene_frame: &SceneFrame,
                    assets: &dyn crate::AssetStore| {
        drawer.draw(
            &mut pixels,
            (1, 1),
            RenderFrame {
                ui: &ui,
                scene_frame,
                assets,
            },
        );
        pixels
    };
    for index in 0..100 {
        let asset = format!("image-{index}.png");
        assets.insert(&asset, png.clone());
        assert_eq!(
            draw(&mut drawer, &image_scene(asset.clone()), &assets),
            [255, 0, 0, 255]
        );
        assert_eq!(drawer.images.len(), 1);
        assert!(drawer.images.contains_key(&asset));
    }

    let asset = "repair.png";
    let scene = image_scene(asset.to_owned());
    let placeholder = draw(&mut drawer, &scene, &assets);
    assert_ne!(placeholder, [255, 0, 0, 255]);
    assert!(drawer
        .image_error(asset)
        .unwrap()
        .contains("Could not load image"));

    assets.insert(asset, b"corrupt image".to_vec());
    let Some(CachedImage::Failed { retry_at, .. }) = drawer.images.get_mut(asset) else {
        panic!("missing image should have a retry deadline");
    };
    *retry_at = Instant::now() + Duration::from_secs(3600);
    assert_eq!(draw(&mut drawer, &scene, &assets), placeholder);
    assert!(drawer
        .image_error(asset)
        .unwrap()
        .contains("Could not load image"));

    let retry_now = |drawer: &mut BuiltinSoftwareDrawer| {
        let Some(CachedImage::Failed { retry_at, .. }) = drawer.images.get_mut(asset) else {
            panic!("failed image should have a retry deadline");
        };
        *retry_at = Instant::now();
    };
    retry_now(&mut drawer);
    assert_eq!(draw(&mut drawer, &scene, &assets), placeholder);
    assert!(drawer.image_error(asset).unwrap().contains("decode error"));

    assets.insert(asset, png);
    retry_now(&mut drawer);
    assert_eq!(draw(&mut drawer, &scene, &assets), [255, 0, 0, 255]);
    assert!(drawer.image_error(asset).is_none());

    draw(&mut drawer, &SceneFrame::default(), &assets);
    assert!(drawer.images.is_empty());
}
