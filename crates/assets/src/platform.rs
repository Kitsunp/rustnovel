use crate::model::{PlatformBudget, PlatformTarget, TranscodePreset};

impl PlatformTarget {
    pub fn default_budget(self) -> PlatformBudget {
        match self {
            PlatformTarget::Desktop => PlatformBudget {
                max_total_bytes: 2 * 1024 * 1024 * 1024,
                max_assets: 20_000,
            },
            PlatformTarget::Mobile => PlatformBudget {
                max_total_bytes: 512 * 1024 * 1024,
                max_assets: 10_000,
            },
            PlatformTarget::Web => PlatformBudget {
                max_total_bytes: 256 * 1024 * 1024,
                max_assets: 8_000,
            },
        }
    }

    pub fn default_transcode_preset(self) -> TranscodePreset {
        match self {
            PlatformTarget::Desktop => TranscodePreset {
                target: self,
                image_extension: "png",
                audio_extension: "ogg",
                image_quality: 95,
                audio_bitrate_kbps: 192,
                max_texture_side: 4096,
            },
            PlatformTarget::Mobile => TranscodePreset {
                target: self,
                image_extension: "webp",
                audio_extension: "ogg",
                image_quality: 85,
                audio_bitrate_kbps: 128,
                max_texture_side: 2048,
            },
            PlatformTarget::Web => TranscodePreset {
                target: self,
                image_extension: "webp",
                audio_extension: "mp3",
                image_quality: 80,
                audio_bitrate_kbps: 128,
                max_texture_side: 2048,
            },
        }
    }
}
