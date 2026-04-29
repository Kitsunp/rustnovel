use eframe::egui;

#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum PreviewQuality {
    Draft,
    #[default]
    Balanced,
    High,
}

impl PreviewQuality {
    pub const ALL: &'static [Self] = &[Self::Draft, Self::Balanced, Self::High];

    pub fn label(self) -> &'static str {
        match self {
            Self::Draft => "Draft 640px",
            Self::Balanced => "Balanced 1280px",
            Self::High => "High native",
        }
    }

    pub fn texture_options(self) -> egui::TextureOptions {
        match self {
            Self::Draft => egui::TextureOptions::NEAREST,
            Self::Balanced | Self::High => egui::TextureOptions::LINEAR,
        }
    }

    pub fn max_texture_edge(self) -> Option<usize> {
        match self {
            Self::Draft => Some(640),
            Self::Balanced => Some(1280),
            Self::High => None,
        }
    }

    pub fn scaled_image(
        self,
        size: [usize; 2],
        pixels: &[u8],
    ) -> ([usize; 2], std::borrow::Cow<'_, [u8]>) {
        let Some(max_edge) = self.max_texture_edge() else {
            return (size, std::borrow::Cow::Borrowed(pixels));
        };
        let target = scaled_size_for_max_edge(size, max_edge);
        if target == size {
            return (size, std::borrow::Cow::Borrowed(pixels));
        }
        (
            target,
            std::borrow::Cow::Owned(resize_rgba_nearest(size, pixels, target)),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum StageFit {
    Compact,
    #[default]
    Normal,
    Fill,
}

impl StageFit {
    pub const ALL: &'static [Self] = &[Self::Compact, Self::Normal, Self::Fill];

    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Normal => "Normal",
            Self::Fill => "Fill",
        }
    }

    pub fn max_stage_fill(self) -> f32 {
        match self {
            Self::Compact => 0.72,
            Self::Normal => 0.88,
            Self::Fill => 1.0,
        }
    }
}

pub(crate) fn fit_stage_rect(
    bounds: egui::Rect,
    stage_size: (f32, f32),
    fit: StageFit,
) -> egui::Rect {
    let (stage_w, stage_h) = stage_size;
    let available = bounds.shrink2(egui::vec2(6.0, 6.0));
    if available.width() <= 1.0 || available.height() <= 1.0 {
        return available;
    }

    let stage_aspect = stage_w / stage_h;
    let available_aspect = available.width() / available.height();
    let mut size = if available_aspect > stage_aspect {
        egui::vec2(available.height() * stage_aspect, available.height())
    } else {
        egui::vec2(available.width(), available.width() / stage_aspect)
    };
    size *= fit.max_stage_fill();
    egui::Rect::from_center_size(available.center(), size)
}

pub(crate) fn stage_scale(stage_rect: egui::Rect, stage_size: (f32, f32)) -> f32 {
    let (stage_w, stage_h) = stage_size;
    (stage_rect.width() / stage_w)
        .min(stage_rect.height() / stage_h)
        .max(0.001)
}

pub(crate) fn scaled_size_for_max_edge(size: [usize; 2], max_edge: usize) -> [usize; 2] {
    let [width, height] = size;
    let longest = width.max(height);
    if longest <= max_edge || longest == 0 {
        return size;
    }
    let ratio = max_edge as f32 / longest as f32;
    [
        ((width as f32 * ratio).round() as usize).max(1),
        ((height as f32 * ratio).round() as usize).max(1),
    ]
}

fn resize_rgba_nearest(size: [usize; 2], pixels: &[u8], target: [usize; 2]) -> Vec<u8> {
    let [src_w, src_h] = size;
    let [dst_w, dst_h] = target;
    let mut out = vec![0; dst_w * dst_h * 4];
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return out;
    }
    for y in 0..dst_h {
        let src_y = (y * src_h / dst_h).min(src_h - 1);
        for x in 0..dst_w {
            let src_x = (x * src_w / dst_w).min(src_w - 1);
            let src = (src_y * src_w + src_x) * 4;
            let dst = (y * dst_w + x) * 4;
            if src + 4 <= pixels.len() {
                out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_fit_scales_stage_monotonically() {
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 900.0));
        let compact = fit_stage_rect(bounds, (1280.0, 720.0), StageFit::Compact);
        let normal = fit_stage_rect(bounds, (1280.0, 720.0), StageFit::Normal);
        let fill = fit_stage_rect(bounds, (1280.0, 720.0), StageFit::Fill);
        assert!(compact.width() < normal.width());
        assert!(normal.width() < fill.width());
    }

    #[test]
    fn stage_fit_preserves_aspect_ratio() {
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 1000.0));
        let rect = fit_stage_rect(bounds, (1280.0, 720.0), StageFit::Fill);
        let aspect = rect.width() / rect.height();
        assert!((aspect - (16.0 / 9.0)).abs() < 0.01);
    }

    #[test]
    fn preview_quality_changes_actual_texture_pixels() {
        assert_eq!(scaled_size_for_max_edge([1920, 1080], 640), [640, 360]);
        assert_eq!(scaled_size_for_max_edge([1920, 1080], 1280), [1280, 720]);
        assert_eq!(
            PreviewQuality::High.scaled_image([1920, 1080], &[0; 16]).0,
            [1920, 1080]
        );
    }
}
