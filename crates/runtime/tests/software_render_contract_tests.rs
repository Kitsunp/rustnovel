use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use visual_novel_engine::runtime::{
    ImageFit, LayoutRect, RenderCommand, SceneFrame, UiState, UiView,
};
use vnengine_runtime::render::{BuiltinSoftwareDrawer, RenderFrame, SoftwareDrawStrategy};
use vnengine_runtime::MemoryAssetStore;

#[test]
fn builtin_drawer_renders_png_images_and_visible_text() {
    let mut assets = MemoryAssetStore::default();
    assets.insert("red.png", one_pixel_png([220, 40, 32, 255]));
    let ui = UiState {
        view: UiView::Scene {
            description: "scene".to_string(),
        },
        pending_transition: None,
    };
    let scene_frame = SceneFrame {
        commands: vec![
            RenderCommand::Clear {
                color: "stage.background".to_string(),
            },
            RenderCommand::Image {
                asset: "red.png".to_string(),
                rect: LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 360.0,
                },
                fit: ImageFit::Stretch,
                z: -100,
            },
            RenderCommand::Text {
                text: "HELLO".to_string(),
                style: "dialogue.text".to_string(),
                rect: LayoutRect {
                    x: 32.0,
                    y: 400.0,
                    width: 640.0,
                    height: 200.0,
                },
            },
        ],
        ..SceneFrame::default()
    };
    let mut frame = vec![0; 160 * 90 * 4];
    let mut drawer = BuiltinSoftwareDrawer::new();

    drawer.draw(
        &mut frame,
        (160, 90),
        RenderFrame {
            ui: &ui,
            scene_frame: &scene_frame,
            assets: &assets,
        },
    );

    assert_eq!(pixel(&frame, 160, 12, 12)[0], 220);
    assert!(
        frame
            .chunks_exact(4)
            .skip(160 * 50)
            .take(160 * 25)
            .any(|pixel| pixel[0] > 180 && pixel[1] > 180 && pixel[2] > 180),
        "expected text rendering to draw bright pixels"
    );
}

fn one_pixel_png(rgba: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&rgba, 1, 1, ColorType::Rgba8.into())
        .expect("encode png");
    bytes
}

fn pixel(frame: &[u8], width: u32, x: u32, y: u32) -> &[u8] {
    let idx = ((y * width + x) * 4) as usize;
    &frame[idx..idx + 4]
}
