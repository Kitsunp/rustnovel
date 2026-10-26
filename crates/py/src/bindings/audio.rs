use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::{PyTraverseError, PyVisit};
use std::sync::Arc;
use std::time::Duration;
use visual_novel_engine::{runtime::AudioCommand, AssetId};

use super::engine::PyEngine;

fn fade_duration(seconds: f64, field: &str) -> PyResult<Duration> {
    if !seconds.is_finite() {
        return Err(PyValueError::new_err(format!("{field} must be finite")));
    }
    Duration::try_from_secs_f64(seconds.max(0.0))
        .map_err(|_| PyValueError::new_err(format!("{field} is too large")))
}

fn normalized_volume(volume: Option<f64>) -> PyResult<Option<f32>> {
    volume
        .map(|value| {
            if !value.is_finite() {
                return Err(PyValueError::new_err("volume must be finite"));
            }
            Ok(value.clamp(0.0, 1.0) as f32)
        })
        .transpose()
}

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
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.engine)
    }

    #[pyo3(signature = (resource, r#loop=true, fade_in=0.0, volume=None))]
    fn play_bgm(
        &self,
        py: Python<'_>,
        resource: &str,
        r#loop: bool,
        fade_in: f64,
        volume: Option<f64>,
    ) -> PyResult<()> {
        let fade_in = fade_duration(fade_in, "fade_in")?;
        let volume = normalized_volume(volume)?;
        let mut engine = self.engine.try_borrow_mut(py)?;
        engine.inner.queue_audio_command(AudioCommand::PlayBgm {
            resource: AssetId::from_path(resource),
            path: Arc::from(resource),
            r#loop,
            volume,
            fade_in,
        });
        Ok(())
    }

    #[pyo3(signature = (fade_out=0.0))]
    fn stop_all(&self, py: Python<'_>, fade_out: f64) -> PyResult<()> {
        let fade_out = fade_duration(fade_out, "fade_out")?;
        let mut engine = self.engine.try_borrow_mut(py)?;
        engine
            .inner
            .queue_audio_command(AudioCommand::StopBgm { fade_out });
        engine.inner.queue_audio_command(AudioCommand::StopSfx);
        engine.inner.queue_audio_command(AudioCommand::StopVoice);
        Ok(())
    }

    #[pyo3(signature = (resource, volume=None))]
    fn play_sfx(&self, py: Python<'_>, resource: &str, volume: Option<f64>) -> PyResult<()> {
        let volume = normalized_volume(volume)?;
        let mut engine = self.engine.try_borrow_mut(py)?;
        engine.inner.queue_audio_command(AudioCommand::PlaySfx {
            resource: AssetId::from_path(resource),
            path: Arc::from(resource),
            volume,
        });
        Ok(())
    }

    #[pyo3(signature = (resource, volume=None))]
    fn play_voice(&self, py: Python<'_>, resource: &str, volume: Option<f64>) -> PyResult<()> {
        let volume = normalized_volume(volume)?;
        let mut engine = self.engine.try_borrow_mut(py)?;
        engine.inner.queue_audio_command(AudioCommand::PlayVoice {
            resource: AssetId::from_path(resource),
            path: Arc::from(resource),
            volume,
        });
        Ok(())
    }
}

pub(super) fn audio_commands_to_python(
    py: Python<'_>,
    commands: &[AudioCommand],
) -> PyResult<Py<PyAny>> {
    use pyo3::types::{PyDict, PyList};
    let list = PyList::empty(py);
    for cmd in commands {
        let dict = PyDict::new(py);
        match cmd {
            AudioCommand::PlayBgm {
                resource,
                path,
                r#loop,
                volume,
                fade_in,
            } => {
                dict.set_item("type", "play_bgm")?;
                dict.set_item("resource", resource.as_u64().to_string())?;
                dict.set_item("path", path.as_ref())?;
                dict.set_item("loop", r#loop)?;
                dict.set_item("volume", volume)?;
                dict.set_item("fade_in", fade_in.as_secs_f64())?;
            }
            AudioCommand::StopBgm { fade_out } => {
                dict.set_item("type", "stop_bgm")?;
                dict.set_item("fade_out", fade_out.as_secs_f64())?;
            }
            AudioCommand::PlaySfx {
                resource,
                path,
                volume,
            } => {
                dict.set_item("type", "play_sfx")?;
                dict.set_item("resource", resource.as_u64().to_string())?;
                dict.set_item("path", path.as_ref())?;
                dict.set_item("volume", volume)?;
            }
            AudioCommand::StopSfx => {
                dict.set_item("type", "stop_sfx")?;
            }
            AudioCommand::PlayVoice {
                resource,
                path,
                volume,
            } => {
                dict.set_item("type", "play_voice")?;
                dict.set_item("resource", resource.as_u64().to_string())?;
                dict.set_item("path", path.as_ref())?;
                dict.set_item("volume", volume)?;
            }
            AudioCommand::StopVoice => {
                dict.set_item("type", "stop_voice")?;
            }
        }
        list.append(dict)?;
    }
    Ok(list.into())
}
