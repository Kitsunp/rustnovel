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
fn changing_scenes_releases_old_images_and_failed_image_keys() {
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
    for index in 0..100 {
        let asset = format!("image-{index}.png");
        assets.insert(&asset, png.clone());
        let scene = SceneFrame {
            commands: vec![RenderCommand::Image {
                asset: asset.clone(),
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
        drawer.draw(
            &mut pixels,
            (1, 1),
            RenderFrame {
                ui: &ui,
                scene_frame: &scene,
                assets: &assets,
            },
        );
        assert_eq!(pixels, [255, 0, 0, 255]);
        assert_eq!(drawer.images.len(), 1);
        assert!(drawer.images.contains_key(&asset));
    }
    drawer.failed_images.insert("missing.png".into());
    drawer.draw(
        &mut pixels,
        (1, 1),
        RenderFrame {
            ui: &ui,
            scene_frame: &SceneFrame::default(),
            assets: &assets,
        },
    );
    assert!(drawer.images.is_empty());
    assert!(drawer.failed_images.is_empty());
}
