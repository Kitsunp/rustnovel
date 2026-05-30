use ab_glyph::{point, Font, FontArc, ScaleFont};
use pixels::{Pixels, SurfaceTexture};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use visual_novel_engine::runtime::{ImageFit, LayoutRect, RenderCommand};
use winit::window::Window;

use super::backend::{RenderBackend, RenderFrame};

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
            if let Some(font) = &self.font {
                draw_text_line_with_font(
                    frame,
                    size,
                    font,
                    &line,
                    area.x as f32,
                    y,
                    font_size,
                    color,
                    area,
                );
            } else {
                draw_fallback_text_line(
                    frame, size, &line, area.x, y as u32, font_size, color, area,
                );
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

fn load_system_font() -> Option<FontArc> {
    for path in system_font_candidates() {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if let Ok(font) = FontArc::try_from_vec(bytes) {
            return Some(font);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn system_font_candidates() -> &'static [&'static str] {
    &[
        "C:/Windows/Fonts/segoeui.ttf",
        "C:/Windows/Fonts/arial.ttf",
        "C:/Windows/Fonts/calibri.ttf",
    ]
}

#[cfg(target_os = "macos")]
fn system_font_candidates() -> &'static [&'static str] {
    &[
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Helvetica.ttf",
        "/System/Library/Fonts/SFNS.ttf",
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn system_font_candidates() -> &'static [&'static str] {
    &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    ]
}

#[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
fn system_font_candidates() -> &'static [&'static str] {
    &[]
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

fn draw_text_line_with_font(
    frame: &mut [u8],
    size: (u32, u32),
    font: &FontArc,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    color: [u8; 4],
    clip: RectSpec,
) {
    let scaled = font.as_scaled(font_size);
    let baseline = y + scaled.ascent();
    let mut caret_x = x;
    let mut previous = None;
    for ch in text.chars() {
        if ch == '\t' {
            caret_x += scaled.h_advance(font.glyph_id(' ')) * 4.0;
            continue;
        }
        let glyph_id = font.glyph_id(ch);
        if let Some(previous) = previous {
            caret_x += scaled.kern(previous, glyph_id);
        }
        let glyph = glyph_id.with_scale_and_position(font_size, point(caret_x, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|px, py, coverage| {
                let x = px as i32 + bounds.min.x.floor() as i32;
                let y = py as i32 + bounds.min.y.floor() as i32;
                if x >= clip.x as i32
                    && y >= clip.y as i32
                    && x < clip.x.saturating_add(clip.width) as i32
                    && y < clip.y.saturating_add(clip.height) as i32
                    && x >= 0
                    && y >= 0
                    && x < size.0 as i32
                    && y < size.1 as i32
                {
                    let mut glyph_color = color;
                    glyph_color[3] = ((color[3] as f32) * coverage).round() as u8;
                    blend_pixel(frame, size.0, x as u32, y as u32, glyph_color);
                }
            });
        }
        caret_x += scaled.h_advance(glyph_id);
        previous = Some(glyph_id);
    }
}

fn draw_fallback_text_line(
    frame: &mut [u8],
    size: (u32, u32),
    text: &str,
    x: u32,
    y: u32,
    font_size: f32,
    color: [u8; 4],
    clip: RectSpec,
) {
    let scale = (font_size / 8.0).round().max(1.0) as u32;
    let mut cursor = x;
    for ch in text.chars() {
        let glyph = fallback_glyph(ch);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                let px = cursor + col * scale;
                let py = y + row as u32 * scale;
                draw_rect(
                    frame,
                    size,
                    RectSpec {
                        x: px.max(clip.x),
                        y: py.max(clip.y),
                        width: scale.min(clip.x.saturating_add(clip.width).saturating_sub(px)),
                        height: scale.min(clip.y.saturating_add(clip.height).saturating_sub(py)),
                        color,
                    },
                );
            }
        }
        cursor = cursor.saturating_add(scale * 6);
        if cursor >= clip.x.saturating_add(clip.width) {
            break;
        }
    }
}

fn wrap_text(text: &str, width: f32, font_size: f32) -> Vec<String> {
    let max_chars = ((width / (font_size * 0.56).max(4.0)).floor() as usize).max(1);
    let mut lines = Vec::new();
    for source_line in text.lines() {
        let mut line = String::new();
        for word in source_line.split_whitespace() {
            let projected = if line.is_empty() {
                word.len()
            } else {
                line.len() + 1 + word.len()
            };
            if projected > max_chars && !line.is_empty() {
                lines.push(line);
                line = String::new();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn scaled_font_size(style: &str, size: (u32, u32)) -> f32 {
    let base = match style {
        "dialogue.speaker" => 22.0,
        "dialogue.text" => 24.0,
        "choice.prompt" => 22.0,
        "button.text" => 18.0,
        "system.warning" => 16.0,
        _ => 18.0,
    };
    let scale = (size.1 as f32 / DESIGN_HEIGHT).clamp(0.65, 2.0);
    base * scale
}

fn resolve_color(name: &str) -> [u8; 4] {
    match name {
        "stage.background" => [22, 24, 34, 255],
        _ => [30, 32, 44, 255],
    }
}

fn resolve_panel_color(style: &str) -> [u8; 4] {
    match style {
        "dialogue_box" => [12, 12, 18, 224],
        "choice_list" => [14, 22, 32, 232],
        "system_overlay" => [64, 18, 24, 224],
        "button.primary" => [72, 112, 198, 235],
        "button.choice" => [38, 112, 118, 235],
        _ => [28, 32, 44, 210],
    }
}

fn resolve_text_color(style: &str) -> [u8; 4] {
    match style {
        "dialogue.speaker" => [255, 220, 142, 255],
        "system.warning" => [255, 214, 128, 255],
        _ => [242, 246, 255, 255],
    }
}

fn fallback_glyph(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        ',' => [0, 0, 0, 0, 0, 0b01100, 0b01000],
        ':' => [0, 0b01100, 0b01100, 0, 0, 0b01100, 0b01100],
        '-' => [0, 0, 0, 0b11110, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        _ => [
            0b11111, 0b10001, 0b00101, 0b01001, 0b10001, 0b10001, 0b11111,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryAssetStore;
    use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
    use visual_novel_engine::runtime::{SceneFrame, UiState, UiView};

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
        let mut drawer = BuiltinSoftwareDrawer {
            font: None,
            images: HashMap::new(),
            failed_images: HashSet::new(),
        };

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
            "expected fallback text to draw bright pixels"
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
}
