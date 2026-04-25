use pixels::{Pixels, SurfaceTexture};
use std::sync::Arc;
use visual_novel_engine::{UiState, UiView};
use winit::window::Window;

use super::backend::RenderBackend;

/// Trait for the actual drawing logic acting on a framebuffer.
pub trait SoftwareDrawStrategy {
    fn draw(&self, frame: &mut [u8], size: (u32, u32), ui: &UiState);
}

/// Backend that uses `pixels` (software rasterization) to display the frame.
pub struct SoftwareBackend<'a> {
    pixels: Pixels<'a>,
    strategy: Box<dyn SoftwareDrawStrategy>,
}

impl<'a> SoftwareBackend<'a> {
    pub fn new(
        window: Arc<Window>,
        width: u32,
        height: u32,
        strategy: Box<dyn SoftwareDrawStrategy>,
    ) -> Self {
        let surface = SurfaceTexture::new(width, height, window);
        let pixels = Pixels::new(width, height, surface).expect("failed to create pixel surface");
        Self { pixels, strategy }
    }
}

impl<'a> RenderBackend for SoftwareBackend<'a> {
    fn resize(&mut self, width: u32, height: u32) {
        let _ = self.pixels.resize_surface(width, height);
        let _ = self.pixels.resize_buffer(width, height);
    }

    fn render(&mut self, ui: &UiState) -> Result<(), String> {
        let extent = self.pixels.context().texture_extent;
        let frame = self.pixels.frame_mut();
        self.strategy.draw(frame, (extent.width, extent.height), ui);

        self.pixels.render().map_err(|e| e.to_string())
    }
}

/// Default implementation of software drawing.
#[derive(Default)]
pub struct BuiltinSoftwareDrawer;

impl SoftwareDrawStrategy for BuiltinSoftwareDrawer {
    fn draw(&self, frame: &mut [u8], size: (u32, u32), ui: &UiState) {
        let (width, height) = size;
        let background = match &ui.view {
            UiView::Dialogue { .. } => [32, 32, 64, 255],
            UiView::Choice { .. } => [24, 48, 48, 255],
            UiView::Scene { .. } => [48, 24, 48, 255],
            UiView::System { .. } => [48, 48, 48, 255],
        };
        clear(frame, background);

        let dialog_height = height / 3;
        let dialog_y = height.saturating_sub(dialog_height + 16);
        match &ui.view {
            UiView::Dialogue { .. } | UiView::Choice { .. } => {
                draw_rect(
                    frame,
                    (width, height),
                    RectSpec {
                        x: 16,
                        y: dialog_y,
                        width: width.saturating_sub(32),
                        height: dialog_height,
                        color: [12, 12, 12, 220],
                    },
                );
            }
            UiView::Scene { .. } => {
                draw_rect(
                    frame,
                    (width, height),
                    RectSpec {
                        x: 16,
                        y: 16,
                        width: width.saturating_sub(32),
                        height: height.saturating_sub(32),
                        color: [20, 20, 20, 180],
                    },
                );
            }
            UiView::System { .. } => {
                draw_rect(
                    frame,
                    (width, height),
                    RectSpec {
                        x: 16,
                        y: 16,
                        width: width.saturating_sub(32),
                        height: 48,
                        color: [96, 16, 16, 200],
                    },
                );
            }
        }

        if let UiView::Choice { options, .. } = &ui.view {
            let option_height = 24;
            let mut y = dialog_y + 16;
            for _ in options {
                draw_rect(
                    frame,
                    (width, height),
                    RectSpec {
                        x: 32,
                        y,
                        width: width.saturating_sub(64),
                        height: option_height,
                        color: [40, 120, 120, 220],
                    },
                );
                y = y.saturating_add(option_height + 8);
            }
        }
    }
}

fn clear(frame: &mut [u8], color: [u8; 4]) {
    for chunk in frame.chunks_exact_mut(4) {
        chunk.copy_from_slice(&color);
    }
}

struct RectSpec {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
}

fn draw_rect(frame: &mut [u8], size: (u32, u32), rect: RectSpec) {
    let (width, height) = size;
    let max_x = (rect.x + rect.width).min(width);
    let max_y = (rect.y + rect.height).min(height);
    for row in rect.y..max_y {
        for col in rect.x..max_x {
            let idx = ((row * width + col) * 4) as usize;
            if idx + 4 <= frame.len() {
                frame[idx..idx + 4].copy_from_slice(&rect.color);
            }
        }
    }
}
