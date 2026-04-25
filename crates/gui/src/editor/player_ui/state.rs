use super::render::byte_index_for_char;
use visual_novel_engine::{Engine, EventCompiled};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipMode {
    Off,
    ReadOnly,
    All,
}

#[derive(Clone, Debug)]
pub struct PlayerSessionState {
    pub show_backlog: bool,
    pub show_choice_history: bool,
    pub autoplay_enabled: bool,
    pub autoplay_delay_ms: u64,
    pub text_chars_per_second: f32,
    pub skip_mode: SkipMode,
    pub bgm_volume: f32,
    pub sfx_volume: f32,
    pub voice_volume: f32,
    pub bgm_muted: bool,
    pub sfx_muted: bool,
    pub voice_muted: bool,
    pub last_audio_event: Option<String>,
    pub last_audio_error: Option<String>,
    current_ip: Option<u32>,
    line_started_at_sec: f64,
    last_auto_step_at_sec: Option<f64>,
}

impl Default for PlayerSessionState {
    fn default() -> Self {
        Self {
            show_backlog: false,
            show_choice_history: false,
            autoplay_enabled: false,
            autoplay_delay_ms: 1200,
            text_chars_per_second: 45.0,
            skip_mode: SkipMode::Off,
            bgm_volume: 1.0,
            sfx_volume: 1.0,
            voice_volume: 1.0,
            bgm_muted: false,
            sfx_muted: false,
            voice_muted: false,
            last_audio_event: None,
            last_audio_error: None,
            current_ip: None,
            line_started_at_sec: 0.0,
            last_auto_step_at_sec: None,
        }
    }
}

impl PlayerSessionState {
    pub(crate) fn on_position_changed(&mut self, position: u32, now_sec: f64) -> bool {
        if self.current_ip != Some(position) {
            self.current_ip = Some(position);
            self.line_started_at_sec = now_sec;
            return true;
        }
        false
    }

    fn reset_runtime_progress(&mut self, now_sec: f64) {
        self.current_ip = None;
        self.line_started_at_sec = now_sec;
        self.last_auto_step_at_sec = None;
    }

    pub(crate) fn reset_for_restart(&mut self, now_sec: f64) {
        self.reset_runtime_progress(now_sec);
    }

    pub(crate) fn reveal_current_line(&mut self, text: &str, now_sec: f64) {
        let cps = self.text_chars_per_second.max(1.0) as f64;
        let needed = (text.chars().count() as f64) / cps;
        self.line_started_at_sec = now_sec - needed;
    }

    pub(crate) fn visible_text<'a>(&self, text: &'a str, now_sec: f64) -> &'a str {
        if text.is_empty() {
            return text;
        }
        let cps = self.text_chars_per_second.max(1.0) as f64;
        let elapsed = (now_sec - self.line_started_at_sec).max(0.0);
        let visible_chars = (elapsed * cps).floor() as usize;
        let total_chars = text.chars().count();
        if visible_chars >= total_chars {
            return text;
        }
        let byte_end = byte_index_for_char(text, visible_chars);
        &text[..byte_end]
    }

    pub(crate) fn is_text_fully_revealed(&self, text: &str, now_sec: f64) -> bool {
        self.visible_text(text, now_sec).len() == text.len()
    }

    pub(crate) fn should_skip_current(&self, event: &EventCompiled, engine: &Engine) -> bool {
        match self.skip_mode {
            SkipMode::Off => false,
            SkipMode::ReadOnly => {
                matches!(event, EventCompiled::Dialogue(_)) && engine.is_current_dialogue_read()
            }
            SkipMode::All => !matches!(event, EventCompiled::Choice(_)),
        }
    }

    pub(crate) fn autoplay_ready(&self, now_sec: f64) -> bool {
        if !self.autoplay_enabled {
            return false;
        }
        match self.last_auto_step_at_sec {
            Some(last) => (now_sec - last).max(0.0) >= (self.autoplay_delay_ms as f64) / 1000.0,
            None => true,
        }
    }

    pub(crate) fn mark_auto_step(&mut self, now_sec: f64) {
        self.last_auto_step_at_sec = Some(now_sec);
    }
}
