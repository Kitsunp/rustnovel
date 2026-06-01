use ab_glyph::{point, Font, FontArc, ScaleFont};

use super::{blend_pixel, draw_rect, RectSpec, DESIGN_HEIGHT};

pub(super) fn load_system_font() -> Option<FontArc> {
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

#[derive(Clone, Copy)]
pub(super) struct TextLineSpec<'a> {
    pub(super) text: &'a str,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) font_size: f32,
    pub(super) color: [u8; 4],
    pub(super) clip: RectSpec,
}

pub(super) fn draw_text_line_with_font(
    frame: &mut [u8],
    size: (u32, u32),
    font: &FontArc,
    spec: TextLineSpec<'_>,
) {
    let scaled = font.as_scaled(spec.font_size);
    let baseline = spec.y + scaled.ascent();
    let mut caret_x = spec.x;
    let mut previous = None;
    for ch in spec.text.chars() {
        if ch == '\t' {
            caret_x += scaled.h_advance(font.glyph_id(' ')) * 4.0;
            continue;
        }
        let glyph_id = font.glyph_id(ch);
        if let Some(previous) = previous {
            caret_x += scaled.kern(previous, glyph_id);
        }
        let glyph = glyph_id.with_scale_and_position(spec.font_size, point(caret_x, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|px, py, coverage| {
                let x = px as i32 + bounds.min.x.floor() as i32;
                let y = py as i32 + bounds.min.y.floor() as i32;
                if x >= spec.clip.x as i32
                    && y >= spec.clip.y as i32
                    && x < spec.clip.x.saturating_add(spec.clip.width) as i32
                    && y < spec.clip.y.saturating_add(spec.clip.height) as i32
                    && x >= 0
                    && y >= 0
                    && x < size.0 as i32
                    && y < size.1 as i32
                {
                    let mut glyph_color = spec.color;
                    glyph_color[3] = ((spec.color[3] as f32) * coverage).round() as u8;
                    blend_pixel(frame, size.0, x as u32, y as u32, glyph_color);
                }
            });
        }
        caret_x += scaled.h_advance(glyph_id);
        previous = Some(glyph_id);
    }
}

pub(super) fn draw_fallback_text_line(frame: &mut [u8], size: (u32, u32), spec: TextLineSpec<'_>) {
    let scale = (spec.font_size / 8.0).round().max(1.0) as u32;
    let mut cursor = spec.x.max(0.0) as u32;
    let y = spec.y.max(0.0) as u32;
    for ch in spec.text.chars() {
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
                        x: px.max(spec.clip.x),
                        y: py.max(spec.clip.y),
                        width: scale.min(
                            spec.clip
                                .x
                                .saturating_add(spec.clip.width)
                                .saturating_sub(px),
                        ),
                        height: scale.min(
                            spec.clip
                                .y
                                .saturating_add(spec.clip.height)
                                .saturating_sub(py),
                        ),
                        color: spec.color,
                    },
                );
            }
        }
        cursor = cursor.saturating_add(scale * 6);
        if cursor >= spec.clip.x.saturating_add(spec.clip.width) {
            break;
        }
    }
}

pub(super) fn wrap_text(text: &str, width: f32, font_size: f32) -> Vec<String> {
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

pub(super) fn scaled_font_size(style: &str, size: (u32, u32)) -> f32 {
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

pub(super) fn resolve_color(name: &str) -> [u8; 4] {
    match name {
        "stage.background" => [22, 24, 34, 255],
        _ => [30, 32, 44, 255],
    }
}

pub(super) fn resolve_panel_color(style: &str) -> [u8; 4] {
    match style {
        "dialogue_box" => [12, 12, 18, 224],
        "choice_list" => [14, 22, 32, 232],
        "system_overlay" => [64, 18, 24, 224],
        "button.primary" => [72, 112, 198, 235],
        "button.choice" => [38, 112, 118, 235],
        _ => [28, 32, 44, 210],
    }
}

pub(super) fn resolve_text_color(style: &str) -> [u8; 4] {
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
