use serde::{Deserialize, Serialize};

use super::TypographyToken;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeColorRgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeColorPreview {
    pub code: String,
    pub rgba: Option<ThemeColorRgba>,
    pub valid: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TypographyPreview {
    pub font_family: String,
    pub sample: String,
    pub size: f32,
    pub weight: u16,
    pub line_height: f32,
    pub line_height_px: f32,
    pub estimated_width: f32,
    pub estimated_height: f32,
}

pub fn parse_theme_color_code(value: &str) -> Result<ThemeColorRgba, String> {
    let code = value.trim();
    let hex = code
        .strip_prefix('#')
        .ok_or_else(|| "color must start with '#'".to_string())?;
    if hex.len() != 6 && hex.len() != 8 {
        return Err("color must be #RRGGBB or #RRGGBBAA".to_string());
    }
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("color contains non-hex characters".to_string());
    }
    Ok(ThemeColorRgba {
        r: parse_hex_pair(hex, 0),
        g: parse_hex_pair(hex, 2),
        b: parse_hex_pair(hex, 4),
        a: if hex.len() == 8 {
            parse_hex_pair(hex, 6)
        } else {
            255
        },
    })
}

pub fn format_theme_color_code(color: ThemeColorRgba) -> String {
    format_theme_color_code_with_alpha(color, color.a < 255)
}

pub fn format_theme_color_code_with_alpha(color: ThemeColorRgba, include_alpha: bool) -> String {
    if include_alpha {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.r, color.g, color.b, color.a
        )
    } else {
        format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
    }
}

pub fn preview_theme_color(value: &str) -> ThemeColorPreview {
    match parse_theme_color_code(value) {
        Ok(rgba) => ThemeColorPreview {
            code: format_theme_color_code(rgba),
            rgba: Some(rgba),
            valid: true,
            error: None,
        },
        Err(error) => ThemeColorPreview {
            code: value.trim().to_string(),
            rgba: None,
            valid: false,
            error: Some(error),
        },
    }
}

pub fn preview_typography_token(token: &TypographyToken, sample: &str) -> TypographyPreview {
    let sample = if sample.trim().is_empty() {
        "The quick brown fox".to_string()
    } else {
        sample.to_string()
    };
    let size = token.size.max(1.0);
    let line_height = token.line_height.max(0.1);
    let weight_factor = (f32::from(token.weight).clamp(100.0, 900.0) - 400.0) / 2500.0;
    let estimated_width = sample.chars().count() as f32 * size * (0.52 + weight_factor.max(0.0));
    let line_height_px = size * line_height;
    TypographyPreview {
        font_family: token.font_family.clone(),
        sample,
        size,
        weight: token.weight,
        line_height,
        line_height_px,
        estimated_width,
        estimated_height: line_height_px,
    }
}

fn parse_hex_pair(hex: &str, start: usize) -> u8 {
    u8::from_str_radix(&hex[start..start + 2], 16).expect("hex pair validated before parse")
}
