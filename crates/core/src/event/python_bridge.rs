use super::{
    python_bridge_helpers::{
        scene_compiled_to_python, scene_patch_add_compiled_to_python, scene_patch_add_to_python,
        scene_patch_remove_compiled_to_python, scene_patch_remove_to_python,
        scene_patch_update_compiled_to_python, scene_patch_update_to_python, scene_to_python,
    },
    EventCompiled, EventRaw,
};

#[derive(Clone, Debug)]
enum PyEventData {
    Raw(EventRaw),
    Compiled(EventCompiled),
}

#[pyo3::pyclass(unsendable)]
#[derive(Debug)]
pub struct PyEvent {
    data: PyEventData,
    cached_dict: std::cell::RefCell<Option<pyo3::PyObject>>,
    cached_options: std::cell::RefCell<Option<pyo3::PyObject>>,
    cached_characters: std::cell::RefCell<Option<pyo3::PyObject>>,
    cached_add: std::cell::RefCell<Option<pyo3::PyObject>>,
    cached_update: std::cell::RefCell<Option<pyo3::PyObject>>,
    cached_remove: std::cell::RefCell<Option<pyo3::PyObject>>,
}

impl PyEvent {
    pub(super) fn from_raw(event: EventRaw) -> Self {
        Self {
            data: PyEventData::Raw(event),
            cached_dict: std::cell::RefCell::new(None),
            cached_options: std::cell::RefCell::new(None),
            cached_characters: std::cell::RefCell::new(None),
            cached_add: std::cell::RefCell::new(None),
            cached_update: std::cell::RefCell::new(None),
            cached_remove: std::cell::RefCell::new(None),
        }
    }

    pub(super) fn from_compiled(event: EventCompiled) -> Self {
        Self {
            data: PyEventData::Compiled(event),
            cached_dict: std::cell::RefCell::new(None),
            cached_options: std::cell::RefCell::new(None),
            cached_characters: std::cell::RefCell::new(None),
            cached_add: std::cell::RefCell::new(None),
            cached_update: std::cell::RefCell::new(None),
            cached_remove: std::cell::RefCell::new(None),
        }
    }

    fn event_type(&self) -> &'static str {
        match &self.data {
            PyEventData::Raw(event) => match event {
                EventRaw::Dialogue(_) => "dialogue",
                EventRaw::Choice(_) => "choice",
                EventRaw::Scene(_) => "scene",
                EventRaw::Jump { .. } => "jump",
                EventRaw::SetFlag { .. } => "set_flag",
                EventRaw::SetVar { .. } => "set_var",
                EventRaw::JumpIf { .. } => "jump_if",
                EventRaw::Patch(_) => "patch",
                EventRaw::ExtCall { .. } => "ext_call",
                EventRaw::AudioAction(_) => "audio_action",
                EventRaw::Transition(_) => "transition",
                EventRaw::SetCharacterPosition(_) => "set_character_position",
            },
            PyEventData::Compiled(event) => match event {
                EventCompiled::Dialogue(_) => "dialogue",
                EventCompiled::Choice(_) => "choice",
                EventCompiled::Scene(_) => "scene",
                EventCompiled::Jump { .. } => "jump",
                EventCompiled::SetFlag { .. } => "set_flag",
                EventCompiled::SetVar { .. } => "set_var",
                EventCompiled::JumpIf { .. } => "jump_if",
                EventCompiled::Patch(_) => "patch",
                EventCompiled::ExtCall { .. } => "ext_call",
                EventCompiled::AudioAction(_) => "audio_action",
                EventCompiled::Transition(_) => "transition",
                EventCompiled::SetCharacterPosition(_) => "set_character_position",
            },
        }
    }

    fn build_dict(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::PyObject> {
        use pyo3::types::{PyDict, PyDictMethods};
        let dict = PyDict::new(py);
        dict.set_item("type", self.event_type())?;
        if let Some(value) = self.speaker_value() {
            dict.set_item("speaker", value)?;
        }
        if let Some(value) = self.text_value() {
            dict.set_item("text", value)?;
        }
        if let Some(value) = self.prompt_value() {
            dict.set_item("prompt", value)?;
        }
        if let Some(options) = self.options_value(py)? {
            dict.set_item("options", options)?;
        }
        if let Some(value) = self.background_value() {
            dict.set_item("background", value)?;
        }
        if let Some(value) = self.music_value() {
            dict.set_item("music", value)?;
        }
        if let Some(characters) = self.characters_value(py)? {
            dict.set_item("characters", characters)?;
        }
        if let Some(add) = self.add_value(py)? {
            dict.set_item("add", add)?;
        }
        if let Some(update) = self.update_value(py)? {
            dict.set_item("update", update)?;
        }
        if let Some(remove) = self.remove_value(py)? {
            dict.set_item("remove", remove)?;
        }
        if let Some(value) = self.target_value(py)? {
            dict.set_item("target", value)?;
        }
        if let Some(value) = self.target_ip_value() {
            dict.set_item("target_ip", value)?;
        }
        if let Some(value) = self.key_value(py)? {
            dict.set_item("key", value)?;
        }
        if let Some(value) = self.flag_id_value() {
            dict.set_item("flag_id", value)?;
        }
        if let Some(value) = self.value_flag() {
            dict.set_item("value", value)?;
        }
        if let Some(value) = self.command_value() {
            dict.set_item("command", value)?;
        }
        if let Some(value) = self.args_value(py)? {
            dict.set_item("args", value)?;
        }
        Ok(dict.into())
    }

    fn speaker_value(&self) -> Option<&str> {
        match &self.data {
            PyEventData::Raw(EventRaw::Dialogue(dialogue)) => Some(dialogue.speaker.as_str()),
            PyEventData::Compiled(EventCompiled::Dialogue(dialogue)) => {
                Some(dialogue.speaker.as_ref())
            }
            _ => None,
        }
    }

    fn text_value(&self) -> Option<&str> {
        match &self.data {
            PyEventData::Raw(EventRaw::Dialogue(dialogue)) => Some(dialogue.text.as_str()),
            PyEventData::Compiled(EventCompiled::Dialogue(dialogue)) => {
                Some(dialogue.text.as_ref())
            }
            _ => None,
        }
    }

    fn prompt_value(&self) -> Option<&str> {
        match &self.data {
            PyEventData::Raw(EventRaw::Choice(choice)) => Some(choice.prompt.as_str()),
            PyEventData::Compiled(EventCompiled::Choice(choice)) => Some(choice.prompt.as_ref()),
            _ => None,
        }
    }

    fn background_value(&self) -> Option<&str> {
        match &self.data {
            PyEventData::Raw(EventRaw::Scene(scene)) => scene.background.as_deref(),
            PyEventData::Compiled(EventCompiled::Scene(scene)) => scene.background.as_deref(),
            PyEventData::Raw(EventRaw::Patch(patch)) => patch.background.as_deref(),
            PyEventData::Compiled(EventCompiled::Patch(patch)) => patch.background.as_deref(),
            _ => None,
        }
    }

    fn music_value(&self) -> Option<&str> {
        match &self.data {
            PyEventData::Raw(EventRaw::Scene(scene)) => scene.music.as_deref(),
            PyEventData::Compiled(EventCompiled::Scene(scene)) => scene.music.as_deref(),
            PyEventData::Raw(EventRaw::Patch(patch)) => patch.music.as_deref(),
            PyEventData::Compiled(EventCompiled::Patch(patch)) => patch.music.as_deref(),
            _ => None,
        }
    }

    fn target_ip_value(&self) -> Option<u32> {
        match &self.data {
            PyEventData::Compiled(EventCompiled::Jump { target_ip }) => Some(*target_ip),
            _ => None,
        }
    }

    fn flag_id_value(&self) -> Option<u32> {
        match &self.data {
            PyEventData::Compiled(EventCompiled::SetFlag { flag_id, .. }) => Some(*flag_id),
            _ => None,
        }
    }

    fn value_flag(&self) -> Option<bool> {
        match &self.data {
            PyEventData::Raw(EventRaw::SetFlag { value, .. }) => Some(*value),
            PyEventData::Compiled(EventCompiled::SetFlag { value, .. }) => Some(*value),
            _ => None,
        }
    }

    fn command_value(&self) -> Option<&str> {
        match &self.data {
            PyEventData::Raw(EventRaw::ExtCall { command, .. }) => Some(command.as_str()),
            PyEventData::Compiled(EventCompiled::ExtCall { command, .. }) => Some(command.as_str()),
            _ => None,
        }
    }

    fn args_value(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        use pyo3::types::{PyList, PyListMethods};
        match &self.data {
            PyEventData::Raw(EventRaw::ExtCall { args, .. }) => {
                let list = PyList::empty(py);
                for arg in args {
                    list.append(arg)?;
                }
                Ok(Some(list.into()))
            }
            PyEventData::Compiled(EventCompiled::ExtCall { args, .. }) => {
                let list = PyList::empty(py);
                for arg in args {
                    list.append(arg)?;
                }
                Ok(Some(list.into()))
            }
            _ => Ok(None),
        }
    }

    fn target_value(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        use pyo3::IntoPyObject;
        match &self.data {
            PyEventData::Raw(EventRaw::Jump { target }) => {
                Ok(Some(target.as_str().into_pyobject(py)?.into_any().unbind()))
            }
            PyEventData::Compiled(EventCompiled::Jump { target_ip }) => {
                Ok(Some(target_ip.into_pyobject(py)?.into_any().unbind()))
            }
            _ => Ok(None),
        }
    }

    fn key_value(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        use pyo3::IntoPyObject;
        match &self.data {
            PyEventData::Raw(EventRaw::SetFlag { key, .. }) => {
                Ok(Some(key.as_str().into_pyobject(py)?.into_any().unbind()))
            }
            PyEventData::Compiled(EventCompiled::SetFlag { flag_id, .. }) => {
                Ok(Some(flag_id.into_pyobject(py)?.into_any().unbind()))
            }
            _ => Ok(None),
        }
    }

    fn options_value(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        if let Some(cached) = self.cached_options.borrow().as_ref() {
            return Ok(Some(cached.clone_ref(py)));
        }
        let list: Option<pyo3::PyObject> = match &self.data {
            PyEventData::Raw(EventRaw::Choice(choice)) => {
                use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
                let options = PyList::empty(py);
                for option in &choice.options {
                    let option_dict = PyDict::new(py);
                    option_dict.set_item("text", option.text.as_str())?;
                    option_dict.set_item("target", option.target.as_str())?;
                    options.append(option_dict)?;
                }
                Some(options.into())
            }
            PyEventData::Compiled(EventCompiled::Choice(choice)) => {
                use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
                let options = PyList::empty(py);
                for option in &choice.options {
                    let option_dict = PyDict::new(py);
                    option_dict.set_item("text", option.text.as_ref())?;
                    option_dict.set_item("target", option.target_ip)?;
                    option_dict.set_item("target_ip", option.target_ip)?;
                    options.append(option_dict)?;
                }
                Some(options.into())
            }
            _ => None,
        };
        if let Some(list) = list {
            *self.cached_options.borrow_mut() = Some(list.clone_ref(py));
            *self.cached_dict.borrow_mut() = None;
            return Ok(Some(list));
        }
        Ok(None)
    }

    fn characters_value(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        if let Some(cached) = self.cached_characters.borrow().as_ref() {
            return Ok(Some(cached.clone_ref(py)));
        }
        let list: Option<pyo3::PyObject> = match &self.data {
            PyEventData::Raw(EventRaw::Scene(scene)) => Some(scene_to_python(py, scene)?),
            PyEventData::Compiled(EventCompiled::Scene(scene)) => {
                Some(scene_compiled_to_python(py, scene)?)
            }
            _ => None,
        };
        if let Some(list) = list {
            *self.cached_characters.borrow_mut() = Some(list.clone_ref(py));
            *self.cached_dict.borrow_mut() = None;
            return Ok(Some(list));
        }
        Ok(None)
    }

    fn add_value(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        if let Some(cached) = self.cached_add.borrow().as_ref() {
            return Ok(Some(cached.clone_ref(py)));
        }
        let list: Option<pyo3::PyObject> = match &self.data {
            PyEventData::Raw(EventRaw::Patch(patch)) => Some(scene_patch_add_to_python(py, patch)?),
            PyEventData::Compiled(EventCompiled::Patch(patch)) => {
                Some(scene_patch_add_compiled_to_python(py, patch)?)
            }
            _ => None,
        };
        if let Some(list) = list {
            *self.cached_add.borrow_mut() = Some(list.clone_ref(py));
            *self.cached_dict.borrow_mut() = None;
            return Ok(Some(list));
        }
        Ok(None)
    }

    fn update_value(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        if let Some(cached) = self.cached_update.borrow().as_ref() {
            return Ok(Some(cached.clone_ref(py)));
        }
        let list: Option<pyo3::PyObject> = match &self.data {
            PyEventData::Raw(EventRaw::Patch(patch)) => {
                Some(scene_patch_update_to_python(py, patch)?)
            }
            PyEventData::Compiled(EventCompiled::Patch(patch)) => {
                Some(scene_patch_update_compiled_to_python(py, patch)?)
            }
            _ => None,
        };
        if let Some(list) = list {
            *self.cached_update.borrow_mut() = Some(list.clone_ref(py));
            *self.cached_dict.borrow_mut() = None;
            return Ok(Some(list));
        }
        Ok(None)
    }

    fn remove_value(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        if let Some(cached) = self.cached_remove.borrow().as_ref() {
            return Ok(Some(cached.clone_ref(py)));
        }
        let list: Option<pyo3::PyObject> = match &self.data {
            PyEventData::Raw(EventRaw::Patch(patch)) => {
                Some(scene_patch_remove_to_python(py, patch)?)
            }
            PyEventData::Compiled(EventCompiled::Patch(patch)) => {
                Some(scene_patch_remove_compiled_to_python(py, patch)?)
            }
            _ => None,
        };
        if let Some(list) = list {
            *self.cached_remove.borrow_mut() = Some(list.clone_ref(py));
            *self.cached_dict.borrow_mut() = None;
            return Ok(Some(list));
        }
        Ok(None)
    }
}

#[pyo3::pymethods]
impl PyEvent {
    #[getter]
    fn r#type(&self) -> &str {
        self.event_type()
    }

    #[getter]
    fn speaker(&self) -> Option<&str> {
        self.speaker_value()
    }

    #[getter]
    fn text(&self) -> Option<&str> {
        self.text_value()
    }

    #[getter]
    fn prompt(&self) -> Option<&str> {
        self.prompt_value()
    }

    #[getter]
    fn background(&self) -> Option<&str> {
        self.background_value()
    }

    #[getter]
    fn music(&self) -> Option<&str> {
        self.music_value()
    }

    #[getter]
    fn target(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        self.target_value(py)
    }

    #[getter]
    fn target_ip(&self) -> Option<u32> {
        self.target_ip_value()
    }

    #[getter]
    fn key(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        self.key_value(py)
    }

    #[getter]
    fn flag_id(&self) -> Option<u32> {
        self.flag_id_value()
    }

    #[getter]
    fn value(&self) -> Option<bool> {
        self.value_flag()
    }

    #[getter]
    fn options(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        self.options_value(py)
    }

    #[getter]
    fn characters(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        self.characters_value(py)
    }

    #[getter]
    fn add(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        self.add_value(py)
    }

    #[getter]
    fn update(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        self.update_value(py)
    }

    #[getter]
    fn remove(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<Option<pyo3::PyObject>> {
        self.remove_value(py)
    }

    fn as_dict(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::PyObject> {
        if let Some(cached) = self.cached_dict.borrow().as_ref() {
            return Ok(cached.clone_ref(py));
        }
        let dict = self.build_dict(py)?;
        *self.cached_dict.borrow_mut() = Some(dict.clone_ref(py));
        Ok(dict)
    }

    fn to_dict(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::PyObject> {
        self.as_dict(py)
    }
}
