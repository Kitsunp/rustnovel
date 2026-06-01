use ab_glyph::FontArc;
use pixels::{Pixels, SurfaceTexture};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use visual_novel_engine::runtime::{ImageFit, LayoutRect, RenderCommand};
use winit::window::Window;

use super::backend::{RenderBackend, RenderFrame};

#[path = "software/text.rs"]
mod text;

use text::{
    draw_fallback_text_line, draw_text_line_with_font, load_system_font, resolve_color,
    resolve_panel_color, resolve_text_color, scaled_font_size, wrap_text, TextLineSpec,
};

const DESIGN_WIDTH: f32 = 1280.0;
const DESIGN_HEIGHT: f32 = 720.0;

/// Trait for the actual drawing logic acting on a framebuffer.
pub trait SoftwareDrawStrategy {
    fn draw(&mut self, frame: &mut [u8], size: (u32, u32), render_frame: RenderFrame<'_>);
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
        Self::try_new(window, width, height, strategy)
            .unwrap_or_else(|err| panic!("failed to create pixel surface: {err}"))
    }

    pub fn try_new(
        window: Arc<Window>,
        width: u32,
        height: u32,
        strategy: Box<dyn SoftwareDrawStrategy>,
    ) -> Result<Self, String> {
        let surface = SurfaceTexture::new(width, height, window);
        let pixels = Pixels::new(width, height, surface).map_err(|err| err.to_string())?;
        Ok(Self { pixels, strategy })
    }
}

impl<'a> RenderBackend for SoftwareBackend<'a> {
    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        self.pixels
            .resize_surface(width, height)
            .map_err(|err| format!("software resize surface failed: {err}"))?;
        self.pixels
            .resize_buffer(width, height)
            .map_err(|err| format!("software resize buffer failed: {err}"))?;
        Ok(())
    }

    fn render(&mut self, render_frame: RenderFrame<'_>) -> Result<(), String> {
        let extent = self.pixels.context().texture_extent;
        let frame = self.pixels.frame_mut();
        self.strategy
            .draw(frame, (extent.width, extent.height), render_frame);

        self.pixels.render().map_err(|e| e.to_string())
    }
}

#[derive(Clone)]
struct DecodedImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// Default implementation of software drawing.
pub struct BuiltinSoftwareDrawer {
    font: Option<FontArc>,
    images: HashMap<String, DecodedImage>,
    failed_images: HashSet<String>,
}

impl BuiltinSoftwareDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    fn draw_command(
        &mut self,
        frame: &mut [u8],
        size: (u32, u32),
        command: &RenderCommand,
        render_frame: RenderFrame<'_>,
    ) {
        match command {
            RenderCommand::Clear { color } => clear(frame, resolve_color(color)),
            RenderCommand::Image {
                asset, rect, fit, ..
            } => self.draw_image(frame, size, render_frame, asset, *rect, *fit),
            RenderCommand::Panel { style, rect } => {
                draw_rect(
                    frame,
                    size,
                    scaled_rect(*rect, size, resolve_panel_color(style)),
                );
            }
            RenderCommand::Text { text, style, rect } => {
                self.draw_text(frame, size, text, style, *rect);
            }
            RenderCommand::Button {
                label, style, rect, ..
            } => {
                let scaled = scaled_rect(*rect, size, resolve_panel_color(style));
                draw_rect(frame, size, scaled);
                let inset = 10.0;
                self.draw_text(
                    frame,
                    size,
                    label,
                    "button.text",
                    LayoutRect {
                        x: rect.x + inset,
                        y: rect.y + 8.0,
                        width: (rect.width - inset * 2.0).max(1.0),
                        height: (rect.height - 12.0).max(1.0),
                    },
                );
            }
        }
    }

    fn draw_image(
        &mut self,
        frame: &mut [u8],
        size: (u32, u32),
        render_frame: RenderFrame<'_>,
        asset: &str,
        rect: LayoutRect,
        fit: ImageFit,
    ) {
        let target = scaled_rect(rect, size, [0, 0, 0, 0]);
        if target.width == 0 || target.height == 0 {
            return;
        }

        let image = match self.load_image(render_frame, asset) {
            Some(image) => image.clone(),
            None => {
                draw_rect(
                    frame,
                    size,
                    RectSpec {
                        color: [112, 32, 48, 190],
                        ..target
                    },
                );
                self.draw_text(frame, size, asset, "system.warning", rect);
                return;
            }
        };
        draw_image_nearest(frame, size, target, &image, fit);
    }

    fn draw_text(
        &self,
        frame: &mut [u8],
        size: (u32, u32),
        text: &str,
        style: &str,
        rect: LayoutRect,
    ) {
        let area = scaled_rect(rect, size, [0, 0, 0, 0]);
        if area.width == 0 || area.height == 0 || text.is_empty() {
            return;
        }

        let font_size = scaled_font_size(style, size);
        let color = resolve_text_color(style);
        let line_height = (font_size * 1.28).max(10.0);
        let lines = wrap_text(text, area.width as f32, font_size);
        let mut y = area.y as f32;
        for line in lines {
            if y + line_height > (area.y + area.height) as f32 {
                break;
            }
            let line_spec = TextLineSpec {
                text: &line,
                x: area.x as f32,
                y,
                font_size,
                color,
                clip: area,
            };
            if let Some(font) = &self.font {
                draw_text_line_with_font(frame, size, font, line_spec);
            } else {
                draw_fallback_text_line(frame, size, line_spec);
            }
            y += line_height;
        }
    }

    fn load_image(&mut self, render_frame: RenderFrame<'_>, asset: &str) -> Option<&DecodedImage> {
        if self.images.contains_key(asset) {
            return self.images.get(asset);
        }
        if self.failed_images.contains(asset) {
            return None;
        }

        let bytes = match render_frame.assets.load_bytes(asset) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("software renderer could not load image '{asset}': {err}");
                self.failed_images.insert(asset.to_string());
                return None;
            }
        };
        let rgba = match image::load_from_memory(&bytes) {
            Ok(image) => image.to_rgba8(),
            Err(err) => {
                eprintln!("software renderer could not decode image '{asset}': {err}");
                self.failed_images.insert(asset.to_string());
                return None;
            }
        };
        let (width, height) = rgba.dimensions();
        self.images.insert(
            asset.to_string(),
            DecodedImage {
                width,
                height,
                rgba: rgba.into_raw(),
            },
        );
        self.images.get(asset)
    }
}

impl Default for BuiltinSoftwareDrawer {
    fn default() -> Self {
        Self {
            font: load_system_font(),
            images: HashMap::new(),
            failed_images: HashSet::new(),
        }
    }
}

impl SoftwareDrawStrategy for BuiltinSoftwareDrawer {
    fn draw(&mut self, frame: &mut [u8], size: (u32, u32), render_frame: RenderFrame<'_>) {
        clear(frame, [22, 24, 34, 255]);
        for command in &render_frame.scene_frame.commands {
            self.draw_command(frame, size, command, render_frame);
        }

        if let Some(transition) = &render_frame.ui.pending_transition {
            let alpha = if transition.kind == "dissolve" {
                96
            } else {
                160
            };
            draw_rect(
                frame,
                size,
                RectSpec {
                    x: 0,
                    y: 0,
                    width: size.0,
                    height: size.1,
                    color: [0, 0, 0, alpha],
                },
            );
        }
    }
}

fn clear(frame: &mut [u8], color: [u8; 4]) {
    for chunk in frame.chunks_exact_mut(4) {
        chunk.copy_from_slice(&color);
    }
}

#[derive(Clone, Copy)]
struct RectSpec {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
}

fn scaled_rect(rect: LayoutRect, size: (u32, u32), color: [u8; 4]) -> RectSpec {
    let (width, height) = size;
    let sx = width as f32 / DESIGN_WIDTH;
    let sy = height as f32 / DESIGN_HEIGHT;
    let x = (rect.x * sx).round().clamp(0.0, width as f32) as u32;
    let y = (rect.y * sy).round().clamp(0.0, height as f32) as u32;
    let max_x = ((rect.x + rect.width) * sx)
        .round()
        .clamp(0.0, width as f32) as u32;
    let max_y = ((rect.y + rect.height) * sy)
        .round()
        .clamp(0.0, height as f32) as u32;
    RectSpec {
        x,
        y,
        width: max_x.saturating_sub(x),
        height: max_y.saturating_sub(y),
        color,
    }
}

fn draw_rect(frame: &mut [u8], size: (u32, u32), rect: RectSpec) {
    let (width, height) = size;
    let max_x = rect.x.saturating_add(rect.width).min(width);
    let max_y = rect.y.saturating_add(rect.height).min(height);
    for row in rect.y..max_y {
        for col in rect.x..max_x {
            blend_pixel(frame, width, col, row, rect.color);
        }
    }
}

fn draw_image_nearest(
    frame: &mut [u8],
    size: (u32, u32),
    rect: RectSpec,
    image: &DecodedImage,
    fit: ImageFit,
) {
    if image.width == 0 || image.height == 0 || rect.width == 0 || rect.height == 0 {
        return;
    }
    let plan = image_draw_plan(rect, image.width, image.height, fit);
    if plan.dest.width == 0 || plan.dest.height == 0 {
        return;
    }

    let max_x = plan.dest.x.saturating_add(plan.dest.width).min(size.0);
    let max_y = plan.dest.y.saturating_add(plan.dest.height).min(size.1);
    for row in plan.dest.y..max_y {
        let v = (row - plan.dest.y) as f32 / plan.dest.height as f32;
        let src_y = (plan.src_y + v * plan.src_h)
            .floor()
            .clamp(0.0, (image.height - 1) as f32) as u32;
        for col in plan.dest.x..max_x {
            let u = (col - plan.dest.x) as f32 / plan.dest.width as f32;
            let src_x = (plan.src_x + u * plan.src_w)
                .floor()
                .clamp(0.0, (image.width - 1) as f32) as u32;
            let idx = ((src_y * image.width + src_x) * 4) as usize;
            if idx + 4 <= image.rgba.len() {
                blend_pixel(
                    frame,
                    size.0,
                    col,
                    row,
                    [
                        image.rgba[idx],
                        image.rgba[idx + 1],
                        image.rgba[idx + 2],
                        image.rgba[idx + 3],
                    ],
                );
            }
        }
    }
}

struct ImageDrawPlan {
    dest: RectSpec,
    src_x: f32,
    src_y: f32,
    src_w: f32,
    src_h: f32,
}

fn image_draw_plan(
    rect: RectSpec,
    image_width: u32,
    image_height: u32,
    fit: ImageFit,
) -> ImageDrawPlan {
    let image_w = image_width as f32;
    let image_h = image_height as f32;
    let rect_w = rect.width as f32;
    let rect_h = rect.height as f32;
    match fit {
        ImageFit::Stretch => ImageDrawPlan {
            dest: rect,
            src_x: 0.0,
            src_y: 0.0,
            src_w: image_w,
            src_h: image_h,
        },
        ImageFit::Contain => {
            let scale = (rect_w / image_w).min(rect_h / image_h);
            let dest_w = (image_w * scale).round().max(1.0) as u32;
            let dest_h = (image_h * scale).round().max(1.0) as u32;
            ImageDrawPlan {
                dest: RectSpec {
                    x: rect.x + rect.width.saturating_sub(dest_w) / 2,
                    y: rect.y + rect.height.saturating_sub(dest_h) / 2,
                    width: dest_w.min(rect.width),
                    height: dest_h.min(rect.height),
                    color: rect.color,
                },
                src_x: 0.0,
                src_y: 0.0,
                src_w: image_w,
                src_h: image_h,
            }
        }
        ImageFit::Cover => {
            let scale = (rect_w / image_w).max(rect_h / image_h);
            let src_w = (rect_w / scale).min(image_w);
            let src_h = (rect_h / scale).min(image_h);
            ImageDrawPlan {
                dest: rect,
                src_x: ((image_w - src_w) * 0.5).max(0.0),
                src_y: ((image_h - src_h) * 0.5).max(0.0),
                src_w,
                src_h,
            }
        }
    }
}

fn blend_pixel(frame: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let idx = ((y * width + x) * 4) as usize;
    if idx + 4 > frame.len() {
        return;
    }
    let alpha = color[3] as f32 / 255.0;
    let inverse = 1.0 - alpha;
    frame[idx] = (color[0] as f32 * alpha + frame[idx] as f32 * inverse).round() as u8;
    frame[idx + 1] = (color[1] as f32 * alpha + frame[idx + 1] as f32 * inverse).round() as u8;
    frame[idx + 2] = (color[2] as f32 * alpha + frame[idx + 2] as f32 * inverse).round() as u8;
    frame[idx + 3] = 255;
}
