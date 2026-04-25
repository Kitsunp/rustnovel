use pyo3::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use visual_novel_engine::{AssetId, AudioCommand};

use super::engine::PyEngine;

#[pyclass(name = "AudioController")]
pub struct PyAudio {
    engine: Py<PyEngine>,
}

impl PyAudio {
    pub fn new(_py: Python<'_>, engine: Py<PyEngine>) -> PyResult<Self> {
        Ok(Self { engine })
    }
}

#[pymethods]
impl PyAudio {
    #[pyo3(signature = (resource, r#loop=true, fade_in=0.0, volume=None))]
    fn play_bgm(
        &self,
        py: Python<'_>,
        resource: &str,
        r#loop: bool,
        fade_in: f64,
        volume: Option<f64>,
    ) -> PyResult<()> {
        let mut engine = self.engine.borrow_mut(py);
        engine.inner.queue_audio_command(AudioCommand::PlayBgm {
            resource: AssetId::from_path(resource),
            path: Arc::from(resource),
            r#loop,
            volume: volume.map(|v| v.clamp(0.0, 1.0) as f32),
            fade_in: Duration::from_secs_f64(fade_in.max(0.0)),
        });
        Ok(())
    }

    #[pyo3(signature = (fade_out=0.0))]
    fn stop_all(&self, py: Python<'_>, fade_out: f64) -> PyResult<()> {
        let mut engine = self.engine.borrow_mut(py);
        engine.inner.queue_audio_command(AudioCommand::StopBgm {
            fade_out: Duration::from_secs_f64(fade_out.max(0.0)),
        });
        engine.inner.queue_audio_command(AudioCommand::StopSfx);
        engine.inner.queue_audio_command(AudioCommand::StopVoice);
        Ok(())
    }

    #[pyo3(signature = (resource, volume=None))]
    fn play_sfx(&self, py: Python<'_>, resource: &str, volume: Option<f64>) -> PyResult<()> {
        let mut engine = self.engine.borrow_mut(py);
        engine.inner.queue_audio_command(AudioCommand::PlaySfx {
            resource: AssetId::from_path(resource),
            path: Arc::from(resource),
            volume: volume.map(|v| v.clamp(0.0, 1.0) as f32),
        });
        Ok(())
    }

    #[pyo3(signature = (resource, volume=None))]
    fn play_voice(&self, py: Python<'_>, resource: &str, volume: Option<f64>) -> PyResult<()> {
        let mut engine = self.engine.borrow_mut(py);
        engine.inner.queue_audio_command(AudioCommand::PlayVoice {
            resource: AssetId::from_path(resource),
            path: Arc::from(resource),
            volume: volume.map(|v| v.clamp(0.0, 1.0) as f32),
        });
        Ok(())
    }
}
