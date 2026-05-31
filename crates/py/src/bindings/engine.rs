use super::audio::PyAudio;
use super::conversion::{event_to_python, ui_state_to_python};
use super::types::{vn_error_to_py, PyResourceConfig, PyRouteTree, PySceneFrame};
use pyo3::exceptions::{PyMemoryError, PyNotImplementedError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyDictMethods, PyList, PyListMethods};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use visual_novel_engine::runtime::{
    AudioCommand, Engine as CoreEngine, EventCompiled, EventRaw, ExternalCallOutcome, ScriptRaw,
    UiState,
};
use visual_novel_engine::{ResourceLimiter, SecurityPolicy};

#[derive(Clone, Debug)]
struct CachedAsset {
    data: Arc<[u8]>,
    last_used: u64,
}

#[derive(Debug, Default)]
struct PyAssetCache {
    entries: HashMap<String, CachedAsset>,
    usage_counter: u64,
    allocated_bytes: usize,
    cache_hits: usize,
    cache_misses: usize,
}

impl PyAssetCache {
    fn len(&self) -> usize {
        self.entries.len()
    }

    fn get(&mut self, asset_path: &str) -> Option<Arc<[u8]>> {
        self.usage_counter = self.usage_counter.saturating_add(1);
        let tick = self.usage_counter;
        match self.entries.get_mut(asset_path) {
            Some(entry) => {
                entry.last_used = tick;
                self.cache_hits = self.cache_hits.saturating_add(1);
                Some(entry.data.clone())
            }
            None => {
                self.cache_misses = self.cache_misses.saturating_add(1);
                None
            }
        }
    }

    fn insert(&mut self, asset_path: String, data: &[u8]) {
        self.usage_counter = self.usage_counter.saturating_add(1);
        let data: Arc<[u8]> = Arc::from(data);
        if let Some(old) = self.entries.insert(
            asset_path,
            CachedAsset {
                data: data.clone(),
                last_used: self.usage_counter,
            },
        ) {
            self.allocated_bytes = self.allocated_bytes.saturating_sub(old.data.len());
        }
        self.allocated_bytes = self.allocated_bytes.saturating_add(data.len());
    }

    fn remove(&mut self, asset_path: &str) -> bool {
        if let Some(old) = self.entries.remove(asset_path) {
            self.allocated_bytes = self.allocated_bytes.saturating_sub(old.data.len());
            true
        } else {
            false
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.allocated_bytes = 0;
    }

    fn evict_unused(&mut self, retained_assets: &HashSet<String>) -> usize {
        let stale_assets = self
            .entries
            .keys()
            .filter(|asset_path| !retained_assets.contains(*asset_path))
            .cloned()
            .collect::<Vec<_>>();
        let evicted = stale_assets.len();
        for asset_path in stale_assets {
            self.remove(&asset_path);
        }
        evicted
    }

    fn evict_over_budget(&mut self, max_bytes: usize) -> usize {
        if self.allocated_bytes <= max_bytes {
            return 0;
        }
        let mut candidates = self
            .entries
            .iter()
            .map(|(asset_path, entry)| (asset_path.clone(), entry.last_used))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_, last_used)| *last_used);
        let mut evicted = 0usize;
        for (asset_path, _) in candidates {
            if self.allocated_bytes <= max_bytes {
                break;
            }
            if self.remove(&asset_path) {
                evicted = evicted.saturating_add(1);
            }
        }
        evicted
    }

    fn hit_rate(&self) -> f64 {
        let total = self.cache_hits.saturating_add(self.cache_misses);
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }
}

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
    asset_cache: PyAssetCache,
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
            asset_cache: PyAssetCache::default(),
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
            let event_ip = self.inner.state().position;
            if !self.allowed_ext_call_commands.contains(command.as_str()) {
                let msg = format!("ext_call '{command}' denied by capability policy");
                self.last_ext_call_error = Some(msg.clone());
                return Err(PyRuntimeError::new_err(msg));
            } else if let Some(handler) = &self.handler {
                let handler = handler.clone_ref(py);
                if let Err(e) = handler.call1(py, (command.as_str(), args.clone())) {
                    let msg = format!("ExtCall handler error for '{command}': {e}");
                    self.last_ext_call_error = Some(msg.clone());
                    let err = self
                        .inner
                        .complete_external_call(ExternalCallOutcome::failed(
                            event_ip,
                            command.clone(),
                            msg,
                        ))
                        .expect_err("failed ext_call outcome must not advance");
                    return Err(PyRuntimeError::new_err(err.to_string()));
                }
                self.inner
                    .complete_external_call(ExternalCallOutcome::succeeded(
                        event_ip,
                        command.clone(),
                    ))
                    .map_err(vn_error_to_py)?;
                self.last_audio_commands = self.inner.take_audio_commands();
                self.last_ext_call_error = None;
            } else {
                let msg = format!("ext_call '{command}' requires a registered handler");
                self.last_ext_call_error = Some(msg.clone());
                return Err(PyRuntimeError::new_err(msg));
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
        self.asset_cache.evict_over_budget(self.max_texture_memory);
    }

    fn get_memory_usage<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("max_texture_memory", self.max_texture_memory)?;
        dict.set_item("max_script_bytes", self.resource_limits.max_script_bytes)?;
        dict.set_item("texture_memory_available", false)?;
        dict.set_item(
            "asset_cache_allocated_bytes",
            self.asset_cache.allocated_bytes,
        )?;
        dict.set_item("asset_cache_entries", self.asset_cache.len())?;
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

    fn cache_asset(&mut self, asset_path: &str, data: &[u8]) -> PyResult<()> {
        let asset_path = asset_path.trim();
        if asset_path.is_empty() {
            return Err(PyValueError::new_err("asset_path must not be empty"));
        }
        if data.len() > self.max_texture_memory {
            return Err(PyMemoryError::new_err(format!(
                "asset '{asset_path}' is {} bytes, above the cache budget of {} bytes",
                data.len(),
                self.max_texture_memory
            )));
        }
        self.asset_cache.insert(asset_path.to_string(), data);
        self.asset_cache.evict_over_budget(self.max_texture_memory);
        Ok(())
    }

    fn get_cached_asset<'py>(
        &mut self,
        py: Python<'py>,
        asset_path: &str,
    ) -> PyResult<Option<PyObject>> {
        if let Some(data) = self.asset_cache.get(asset_path) {
            Ok(Some(PyBytes::new(py, data.as_ref()).into()))
        } else {
            Ok(None)
        }
    }

    fn invalidate_cached_asset(&mut self, asset_path: &str) -> bool {
        self.asset_cache.remove(asset_path)
    }

    fn clear_asset_cache(&mut self) {
        self.asset_cache.clear();
    }

    fn evict_unused_assets(&mut self) -> usize {
        let mut retained_assets = HashSet::new();
        let visual_state = self.inner.visual_state();
        if let Some(background) = visual_state.background.as_deref() {
            retained_assets.insert(background.to_string());
        }
        if let Some(music) = visual_state.music.as_deref() {
            retained_assets.insert(music.to_string());
        }
        for character in &visual_state.characters {
            if let Some(expression) = character.expression.as_deref() {
                retained_assets.insert(expression.to_string());
            }
        }
        retained_assets.extend(self.inner.peek_next_asset_paths(self.prefetch_depth));
        self.asset_cache.evict_unused(&retained_assets)
    }

    fn asset_cache_stats<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("total_cached_assets", self.asset_cache.len())?;
        dict.set_item("allocated_bytes", self.asset_cache.allocated_bytes)?;
        dict.set_item("cache_hits", self.asset_cache.cache_hits)?;
        dict.set_item("cache_misses", self.asset_cache.cache_misses)?;
        dict.set_item("hit_rate", self.asset_cache.hit_rate())?;
        Ok(dict.into())
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

    fn pending_external_call<'py>(&self, py: Python<'py>) -> PyResult<Option<PyObject>> {
        let Some(request) = self.inner.pending_external_call().map_err(vn_error_to_py)? else {
            return Ok(None);
        };
        let dict = PyDict::new(py);
        dict.set_item("event_ip", request.event_ip)?;
        dict.set_item("command", request.command)?;
        dict.set_item("args", request.args)?;
        Ok(Some(dict.into()))
    }

    #[pyo3(signature = (success = true, message = None))]
    fn complete_external_call(&mut self, success: bool, message: Option<String>) -> PyResult<()> {
        let request = self
            .inner
            .pending_external_call()
            .map_err(vn_error_to_py)?
            .ok_or_else(|| PyRuntimeError::new_err("no external call is pending"))?;
        let outcome = if success {
            ExternalCallOutcome::succeeded(request.event_ip, request.command)
        } else {
            ExternalCallOutcome::failed(
                request.event_ip,
                request.command,
                message.unwrap_or_else(|| "external call failed".to_string()),
            )
        };
        self.inner
            .complete_external_call(outcome)
            .map_err(vn_error_to_py)
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
