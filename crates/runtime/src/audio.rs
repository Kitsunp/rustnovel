use std::io::Cursor;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rodio::{Decoder, OutputStream, Sink, Source};
use visual_novel_engine::LruCache;

use crate::AssetStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioCapabilities {
    pub bgm_fade: bool,
    pub stop_sfx: bool,
    pub stop_voice: bool,
    pub no_op: bool,
}

impl AudioCapabilities {
    pub const SILENT: Self = Self {
        bgm_fade: false,
        stop_sfx: false,
        stop_voice: false,
        no_op: true,
    };

    pub const RODIO: Self = Self {
        bgm_fade: true,
        stop_sfx: true,
        stop_voice: true,
        no_op: false,
    };
}

/// Audio trait for runtime playback backends.
pub trait Audio {
    fn capabilities(&self) -> AudioCapabilities {
        AudioCapabilities::SILENT
    }
    fn play_music(&mut self, id: &str);
    fn play_music_with_options(&mut self, id: &str, loop_playback: bool, volume: Option<f32>) {
        self.play_music_with_transition(id, loop_playback, volume, None);
    }
    fn play_music_with_transition(
        &mut self,
        id: &str,
        loop_playback: bool,
        volume: Option<f32>,
        fade_in: Option<Duration>,
    ) {
        let _ = fade_in;
        let _ = (loop_playback, volume);
        self.play_music(id);
    }
    fn stop_music(&mut self);
    fn stop_music_with_fade(&mut self, fade_out: Option<Duration>) {
        let _ = fade_out;
        self.stop_music();
    }
    fn play_sfx(&mut self, id: &str);
    fn play_sfx_with_volume(&mut self, id: &str, volume: Option<f32>) {
        let _ = volume;
        self.play_sfx(id);
    }
    fn stop_sfx(&mut self) {}
    fn play_voice_with_volume(&mut self, id: &str, volume: Option<f32>) {
        self.play_sfx_with_volume(id, volume);
    }
    fn stop_voice(&mut self) {}
}

impl<T: Audio + ?Sized> Audio for Box<T> {
    fn capabilities(&self) -> AudioCapabilities {
        (**self).capabilities()
    }
    fn play_music(&mut self, id: &str) {
        (**self).play_music(id);
    }
    fn play_music_with_options(&mut self, id: &str, loop_playback: bool, volume: Option<f32>) {
        (**self).play_music_with_options(id, loop_playback, volume);
    }
    fn play_music_with_transition(
        &mut self,
        id: &str,
        loop_playback: bool,
        volume: Option<f32>,
        fade_in: Option<Duration>,
    ) {
        (**self).play_music_with_transition(id, loop_playback, volume, fade_in);
    }
    fn stop_music(&mut self) {
        (**self).stop_music();
    }
    fn stop_music_with_fade(&mut self, fade_out: Option<Duration>) {
        (**self).stop_music_with_fade(fade_out);
    }
    fn play_sfx(&mut self, id: &str) {
        (**self).play_sfx(id);
    }
    fn play_sfx_with_volume(&mut self, id: &str, volume: Option<f32>) {
        (**self).play_sfx_with_volume(id, volume);
    }
    fn stop_sfx(&mut self) {
        (**self).stop_sfx();
    }
    fn play_voice_with_volume(&mut self, id: &str, volume: Option<f32>) {
        (**self).play_voice_with_volume(id, volume);
    }
    fn stop_voice(&mut self) {
        (**self).stop_voice();
    }
}

/// Audio backend implementation using `rodio`.
///
/// This backend runs audio on a dedicated thread (managed by rodio's OutputStream).
/// It handles decoding and mixing of multiple audio sources.
pub struct RodioBackend {
    _stream: OutputStream,
    stream_handle: rodio::OutputStreamHandle,
    bgm_sink: Arc<Sink>,
    assets: Arc<dyn AssetStore + Send + Sync>,
    audio_cache: LruCache<String>,
    current_bgm: Option<String>,
    sfx_sinks: Vec<Arc<Sink>>,
    voice_sink: Option<Arc<Sink>>,
}

impl RodioBackend {
    const AUDIO_CACHE_BUDGET_BYTES: usize = 64 * 1024 * 1024;

    pub fn new(assets: Arc<dyn AssetStore + Send + Sync>) -> Result<Self, String> {
        let (stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| format!("failed to initialize audio output stream: {}", e))?;

        let bgm_sink = Arc::new(
            Sink::try_new(&stream_handle)
                .map_err(|e| format!("failed to create BGM sink: {}", e))?,
        );

        Ok(Self {
            _stream: stream,
            stream_handle,
            bgm_sink,
            assets,
            audio_cache: LruCache::new(Self::AUDIO_CACHE_BUDGET_BYTES),
            current_bgm: None,
            sfx_sinks: Vec::new(),
            voice_sink: None,
        })
    }

    fn load_audio_bytes_cached(&mut self, id: &str) -> Result<Vec<u8>, String> {
        let key = id.to_string();
        if let Some(cached) = self.audio_cache.get(&key) {
            return Ok(cached.clone());
        }

        let bytes = self.assets.load_bytes(id)?;
        self.audio_cache.insert(key, bytes.clone());
        Ok(bytes)
    }

    fn play_source(
        &mut self,
        source: Box<dyn Source<Item = f32> + Send>,
        is_bgm: bool,
        loop_playback: bool,
        volume: Option<f32>,
        fade_in: Option<Duration>,
    ) {
        if is_bgm {
            if !self.bgm_sink.empty() {
                fade_sink_to_stop(self.bgm_sink.clone(), fade_in);
            }
            self.bgm_sink = match Sink::try_new(&self.stream_handle) {
                Ok(sink) => Arc::new(sink),
                Err(e) => {
                    eprintln!("Failed to create BGM sink: {}", e);
                    return;
                }
            };
            let target_volume = volume.unwrap_or(1.0).clamp(0.0, 1.0);
            self.bgm_sink.set_volume(if fade_in.is_some() {
                0.0
            } else {
                target_volume
            });
            if loop_playback {
                self.bgm_sink.append(source.repeat_infinite());
            } else {
                self.bgm_sink.append(source);
            }
            self.bgm_sink.play();
            fade_sink_to_volume(self.bgm_sink.clone(), target_volume, fade_in);
        } else {
            // SFX - fire and forget, fail-soft on sink creation errors.
            let sink = match Sink::try_new(&self.stream_handle) {
                Ok(sink) => Arc::new(sink),
                Err(e) => {
                    eprintln!("Failed to create SFX sink: {}", e);
                    return;
                }
            };
            if let Some(level) = volume {
                sink.set_volume(level.clamp(0.0, 1.0));
            }
            sink.append(source);
            sink.play();
            self.sfx_sinks.retain(|sink| !sink.empty());
            self.sfx_sinks.push(sink);
        }
    }

    fn play_voice_internal(
        &mut self,
        source: Box<dyn Source<Item = f32> + Send>,
        volume: Option<f32>,
    ) {
        if let Some(existing) = self.voice_sink.take() {
            existing.stop();
        }

        let sink = match Sink::try_new(&self.stream_handle) {
            Ok(sink) => Arc::new(sink),
            Err(e) => {
                eprintln!("Failed to create Voice sink: {}", e);
                return;
            }
        };
        if let Some(level) = volume {
            sink.set_volume(level.clamp(0.0, 1.0));
        }
        sink.append(source);
        sink.play();
        self.voice_sink = Some(sink);
    }
}

impl Audio for RodioBackend {
    fn capabilities(&self) -> AudioCapabilities {
        AudioCapabilities::RODIO
    }

    fn play_music(&mut self, id: &str) {
        self.play_music_with_options(id, true, None);
    }

    fn play_music_with_options(&mut self, id: &str, loop_playback: bool, volume: Option<f32>) {
        self.play_music_with_transition(id, loop_playback, volume, None);
    }

    fn play_music_with_transition(
        &mut self,
        id: &str,
        loop_playback: bool,
        volume: Option<f32>,
        fade_in: Option<Duration>,
    ) {
        if self.current_bgm.as_deref() == Some(id) && !self.bgm_sink.empty() {
            return;
        }

        let data = match self.load_audio_bytes_cached(id) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Audio Error: {}", e);
                return;
            }
        };

        let cursor = Cursor::new(data);
        match Decoder::new(cursor) {
            Ok(decoder) => {
                let source = decoder.convert_samples::<f32>();
                self.play_source(Box::new(source), true, loop_playback, volume, fade_in);
                self.current_bgm = Some(id.to_string());
            }
            Err(e) => eprintln!("Failed to decode music '{}': {}", id, e),
        }
    }

    fn stop_music(&mut self) {
        self.bgm_sink.stop();
        self.current_bgm = None;
    }

    fn stop_music_with_fade(&mut self, fade_out: Option<Duration>) {
        fade_sink_to_stop(self.bgm_sink.clone(), fade_out);
        self.current_bgm = None;
    }

    fn play_sfx(&mut self, id: &str) {
        self.play_sfx_with_volume(id, None);
    }

    fn play_sfx_with_volume(&mut self, id: &str, volume: Option<f32>) {
        let data = match self.load_audio_bytes_cached(id) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Audio Error: {}", e);
                return;
            }
        };

        let cursor = Cursor::new(data);
        match Decoder::new(cursor) {
            Ok(decoder) => {
                let source = decoder.convert_samples::<f32>();
                self.play_source(Box::new(source), false, false, volume, None);
            }
            Err(e) => eprintln!("Failed to decode sfx '{}': {}", id, e),
        }
    }

    fn play_voice_with_volume(&mut self, id: &str, volume: Option<f32>) {
        let data = match self.load_audio_bytes_cached(id) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Audio Error: {}", e);
                return;
            }
        };

        let cursor = Cursor::new(data);
        match Decoder::new(cursor) {
            Ok(decoder) => {
                let source = decoder.convert_samples::<f32>();
                self.play_voice_internal(Box::new(source), volume);
            }
            Err(e) => eprintln!("Failed to decode voice '{}': {}", id, e),
        }
    }

    fn stop_voice(&mut self) {
        if let Some(sink) = self.voice_sink.take() {
            sink.stop();
        }
    }

    fn stop_sfx(&mut self) {
        for sink in self.sfx_sinks.drain(..) {
            sink.stop();
        }
    }
}

fn fade_sink_to_volume(sink: Arc<Sink>, target_volume: f32, fade: Option<Duration>) {
    let Some(fade) = fade.filter(|duration| !duration.is_zero()) else {
        sink.set_volume(target_volume);
        return;
    };
    thread::spawn(move || {
        const STEPS: u32 = 16;
        let sleep = fade / STEPS;
        for step in 1..=STEPS {
            let t = step as f32 / STEPS as f32;
            sink.set_volume(target_volume * t);
            thread::sleep(sleep);
        }
    });
}

fn fade_sink_to_stop(sink: Arc<Sink>, fade: Option<Duration>) {
    let Some(fade) = fade.filter(|duration| !duration.is_zero()) else {
        sink.stop();
        return;
    };
    thread::spawn(move || {
        const STEPS: u32 = 16;
        let start_volume = sink.volume();
        let sleep = fade / STEPS;
        for step in (0..STEPS).rev() {
            let t = step as f32 / STEPS as f32;
            sink.set_volume(start_volume * t);
            thread::sleep(sleep);
        }
        sink.stop();
    });
}

/// No-op audio backend for environments where sound output is disabled/unavailable.
#[derive(Default)]
pub struct SilentAudio;

impl Audio for SilentAudio {
    fn capabilities(&self) -> AudioCapabilities {
        AudioCapabilities::SILENT
    }

    fn play_music(&mut self, _id: &str) {}

    fn stop_music(&mut self) {}

    fn play_sfx(&mut self, _id: &str) {}
}
