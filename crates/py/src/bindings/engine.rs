use super::audio::PyAudio;
use super::conversion::{event_to_python, ui_state_to_python};
use super::types::{vn_error_to_py, PyResourceConfig};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
use std::collections::BTreeSet;
use visual_novel_engine::{
    AudioCommand, Engine as CoreEngine, EventCompiled, EventRaw, ResourceLimiter, ScriptRaw,
    SecurityPolicy, UiState,
};

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
        dict.set_item("current_texture_bytes", 0usize)?;
        dict.set_item("max_texture_memory", self.max_texture_memory)?;
        dict.set_item("max_script_bytes", self.resource_limits.max_script_bytes)?;
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

    fn is_loading(&self) -> bool {
        false
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

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::ffi::c_str;
    use pyo3::types::PyModule;

    fn make_ext_call_engine() -> PyEngine {
        let script_json = r#"{
  "script_schema_version": "1.0",
  "events": [
    { "type": "ext_call", "command": "minigame_start", "args": ["cards"] },
    { "type": "dialogue", "speaker": "Narrator", "text": "Next" }
  ],
  "labels": { "start": 0 }
}"#;
        PyEngine::new(script_json).expect("engine should build")
    }

    #[test]
    fn ext_call_callbacks_are_denied_by_default() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let mut engine = make_ext_call_engine();
            let module = PyModule::from_code(
                py,
                c_str!(
                    r#"
calls = []
def handler(command, args):
    calls.append((command, list(args)))
"#
                ),
                c_str!("handler.py"),
                c_str!("handler_mod"),
            )
            .expect("python module");
            let handler = module.getattr("handler").expect("handler").unbind();

            engine.register_handler(handler);
            let _result = engine.step(py).expect("ext-call step should still succeed");
            assert_eq!(
                module
                    .getattr("calls")
                    .expect("calls list")
                    .extract::<Vec<(String, Vec<String>)>>()
                    .expect("extract calls"),
                Vec::<(String, Vec<String>)>::new()
            );
            assert!(
                engine
                    .last_ext_call_error()
                    .as_deref()
                    .is_some_and(|message| message.contains("denied")),
                "denied ext-call should be recorded"
            );
        });
    }

    #[test]
    fn ext_call_callbacks_require_explicit_authorization() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let mut engine = make_ext_call_engine();
            let module = PyModule::from_code(
                py,
                c_str!(
                    r#"
calls = []
def handler(command, args):
    calls.append((command, list(args)))
"#
                ),
                c_str!("handler.py"),
                c_str!("handler_mod"),
            )
            .expect("python module");
            let handler = module.getattr("handler").expect("handler").unbind();

            engine.allow_ext_call_command("minigame_start");
            engine.register_handler(handler);
            let _ = engine.step(py).expect("authorized ext-call should succeed");

            let calls = module
                .getattr("calls")
                .expect("calls list")
                .extract::<Vec<(String, Vec<String>)>>()
                .expect("extract calls");
            assert_eq!(
                calls,
                vec![("minigame_start".to_string(), vec!["cards".to_string()])]
            );
            assert_eq!(engine.last_ext_call_error(), None);

            engine.resume().expect("resume after ext-call");
            let _ = engine
                .step(py)
                .expect("post-ext-call dialogue should step cleanly");
            assert_eq!(engine.last_ext_call_error(), None);
        });
    }
}
