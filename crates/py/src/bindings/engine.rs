use super::audio::PyAudio;
use super::conversion::{event_to_python, ui_state_to_python};
use super::types::{vn_error_to_py, PyResourceConfig, PyRouteTree, PySceneFrame};
use pyo3::exceptions::PyNotImplementedError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
use std::collections::BTreeSet;
use visual_novel_engine::runtime::{
    AudioCommand, Engine as CoreEngine, EventCompiled, EventRaw, ScriptRaw, UiState,
};
use visual_novel_engine::{ResourceLimiter, SecurityPolicy};

#[pyclass(name = "Engine")]
#[derive(Debug)]
pub struct PyEngine {
    pub(crate) inner: CoreEngine,
    resource_limits: ResourceLimiter,
    max_texture_memory: usize,
    prefetch_depth: usize,
    handler: Option<Py<PyAny>>,
    allowed_ext_call_commands: BTreeSet<String>,
    last_ext_call_error: Option<String>,
    last_audio_commands: Vec<AudioCommand>,
}

#[pyclass]
pub struct StepResult {
    #[pyo3(get)]
    pub event: PyObject,
    #[pyo3(get)]
    pub audio: PyObject,
}

#[pymethods]
impl PyEngine {
    #[new]
    pub fn new(script_json: &str) -> PyResult<Self> {
        let resource_limits = ResourceLimiter::default();
        let script = ScriptRaw::from_json_with_limits(script_json, resource_limits)
            .map_err(vn_error_to_py)?;
        let inner = CoreEngine::new(script, SecurityPolicy::default(), resource_limits)
            .map_err(vn_error_to_py)?;
        Ok(Self {
            inner,
            resource_limits,
            max_texture_memory: 512 * 1024 * 1024,
            prefetch_depth: 0,
            handler: None,
            allowed_ext_call_commands: BTreeSet::new(),
            last_ext_call_error: None,
            last_audio_commands: Vec::new(),
        })
    }

    fn current_event<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let event = self.inner.current_event().map_err(vn_error_to_py)?;
        event_to_python(&event, py)
    }

    fn step<'py>(&mut self, py: Python<'py>) -> PyResult<StepResult> {
        let (audio, change) = self.inner.step().map_err(vn_error_to_py)?;
        self.last_audio_commands = audio;
        let event = change.event;
        if let EventCompiled::ExtCall { command, args } = &event {
            if !self.allowed_ext_call_commands.contains(command.as_str()) {
                self.last_ext_call_error =
                    Some(format!("ext_call '{command}' denied by capability policy"));
            } else if let Some(handler) = &self.handler {
                let handler = handler.clone_ref(py);
                if let Err(e) = handler.call1(py, (command.as_str(), args.clone())) {
                    let msg = format!("ExtCall handler error for '{command}': {e}");
                    self.last_ext_call_error = Some(msg.clone());
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(msg));
                }
                self.last_ext_call_error = None;
            } else {
                self.last_ext_call_error = None;
            }
        } else {
            self.last_ext_call_error = None;
        }
        let event_obj = event_to_python(&event, py)?;
        let audio_obj = self.get_last_audio_commands(py)?;
        Ok(StepResult {
            event: event_obj,
            audio: audio_obj,
        })
    }

    fn choose<'py>(&mut self, py: Python<'py>, option_index: usize) -> PyResult<PyObject> {
        let event = self.inner.choose(option_index).map_err(vn_error_to_py)?;
        self.last_audio_commands = self.inner.take_audio_commands();
        event_to_python(&event, py)
    }

    fn current_event_json(&self) -> PyResult<String> {
        self.inner.current_event_json().map_err(vn_error_to_py)
    }

    fn supported_event_types(&self) -> Vec<&'static str> {
        EventRaw::TYPE_NAMES.to_vec()
    }

    fn visual_state<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let state = self.inner.visual_state();
        let dict = PyDict::new(py);
        dict.set_item("background", state.background.as_deref())?;
        dict.set_item("music", state.music.as_deref())?;
        let characters = PyList::empty(py);
        for character in &state.characters {
            let character_dict = PyDict::new(py);
            character_dict.set_item("name", character.name.as_ref())?;
            character_dict.set_item("expression", character.expression.as_deref())?;
            character_dict.set_item("position", character.position.as_deref())?;
            character_dict.set_item("x", character.x)?;
            character_dict.set_item("y", character.y)?;
            character_dict.set_item("scale", character.scale)?;
            characters.append(character_dict)?;
        }
        dict.set_item("characters", characters)?;
        Ok(dict.into())
    }

    fn is_current_dialogue_read(&self) -> bool {
        self.inner.is_current_dialogue_read()
    }

    fn choice_history<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let list = PyList::empty(py);
        for entry in self.inner.choice_history() {
            let dict = PyDict::new(py);
            dict.set_item("event_ip", entry.event_ip)?;
            dict.set_item("option_index", entry.option_index)?;
            dict.set_item("option_text", entry.option_text.as_str())?;
            dict.set_item("target_ip", entry.target_ip)?;
            list.append(dict)?;
        }
        Ok(list.into())
    }

    fn ui_state<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let event = self.inner.current_event().map_err(vn_error_to_py)?;
        let ui = UiState::from_event(&event, self.inner.visual_state());
        ui_state_to_python(&ui, py)
    }

    fn route_tree(&self) -> PyRouteTree {
        self.inner.route_tree().into()
    }

    fn scene_frame(&self) -> PySceneFrame {
        self.inner.scene_frame().into()
    }

    fn get_last_audio_commands<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let list = PyList::empty(py);
        for cmd in &self.last_audio_commands {
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

    fn set_resources(&mut self, config: PyResourceConfig) {
        self.max_texture_memory = config.max_texture_memory;
        self.resource_limits.max_script_bytes = config.max_script_bytes;
    }

    fn get_memory_usage<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("max_texture_memory", self.max_texture_memory)?;
        dict.set_item("max_script_bytes", self.resource_limits.max_script_bytes)?;
        dict.set_item("texture_memory_available", false)?;
        Ok(dict.into())
    }

    fn set_prefetch_depth(&mut self, depth: usize) {
        self.prefetch_depth = depth;
    }

    fn prefetch_depth(&self) -> usize {
        self.prefetch_depth
    }

    fn prefetch_assets_hint<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let list = PyList::empty(py);
        for path in self.inner.peek_next_asset_paths(self.prefetch_depth) {
            list.append(path)?;
        }
        Ok(list.into())
    }

    fn is_loading(&self) -> PyResult<bool> {
        Err(PyNotImplementedError::new_err(
            "loading state is not available in the headless Python engine",
        ))
    }

    fn register_handler(&mut self, callback: Py<PyAny>) {
        self.handler = Some(callback);
    }

    fn allow_ext_call_command(&mut self, command: &str) {
        self.allowed_ext_call_commands.insert(command.to_string());
    }

    fn clear_ext_call_capabilities(&mut self) {
        self.allowed_ext_call_commands.clear();
    }

    fn last_ext_call_error(&self) -> Option<String> {
        self.last_ext_call_error.clone()
    }

    fn resume(&mut self) -> PyResult<()> {
        self.inner.resume().map_err(vn_error_to_py)?;
        Ok(())
    }

    fn audio(slf: PyRef<'_, Self>) -> PyResult<Py<PyAudio>> {
        let py = slf.py();
        let engine: Py<PyEngine> = slf.into();
        Py::new(py, PyAudio::new(py, engine)?)
    }
}
