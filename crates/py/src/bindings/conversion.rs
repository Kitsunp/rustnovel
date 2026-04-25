use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
use visual_novel_engine::{
    CharacterPatchCompiled, CharacterPlacementCompiled, EventCompiled, SharedStr, UiState, UiView,
};

pub fn event_to_python(event: &EventCompiled, py: Python<'_>) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    match event {
        EventCompiled::Dialogue(dialogue) => {
            dict.set_item("type", "dialogue")?;
            dict.set_item("speaker", dialogue.speaker.as_ref())?;
            dict.set_item("text", dialogue.text.as_ref())?;
        }
        EventCompiled::Choice(choice) => {
            dict.set_item("type", "choice")?;
            dict.set_item("prompt", choice.prompt.as_ref())?;
            let options = PyList::empty(py);
            for option in &choice.options {
                let option_dict = PyDict::new(py);
                option_dict.set_item("text", option.text.as_ref())?;
                option_dict.set_item("target", option.target_ip)?;
                option_dict.set_item("target_ip", option.target_ip)?;
                options.append(option_dict)?;
            }
            dict.set_item("options", options)?;
        }
        EventCompiled::Scene(scene) => {
            dict.set_item("type", "scene")?;
            dict.set_item("background", scene.background.as_deref())?;
            dict.set_item("music", scene.music.as_deref())?;
            let characters = PyList::empty(py);
            for character in &scene.characters {
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
        }
        EventCompiled::Jump { target_ip } => {
            dict.set_item("type", "jump")?;
            dict.set_item("target", *target_ip)?;
            dict.set_item("target_ip", *target_ip)?;
        }
        EventCompiled::SetFlag { flag_id, value } => {
            dict.set_item("type", "set_flag")?;
            dict.set_item("key", *flag_id)?;
            dict.set_item("flag_id", *flag_id)?;
            dict.set_item("value", *value)?;
        }
        EventCompiled::SetVar { var_id, value } => {
            dict.set_item("type", "set_var")?;
            dict.set_item("var_id", *var_id)?;
            dict.set_item("value", *value)?;
        }
        EventCompiled::JumpIf { target_ip, .. } => {
            dict.set_item("type", "jump_if")?;
            dict.set_item("target_ip", *target_ip)?;
        }
        EventCompiled::Patch(patch) => {
            dict.set_item("type", "patch")?;
            dict.set_item("background", patch.background.as_deref())?;
            dict.set_item("music", patch.music.as_deref())?;
            dict.set_item("add", characters_to_python(py, &patch.add)?)?;
            dict.set_item("update", patch_update_to_python(py, &patch.update)?)?;
            dict.set_item("remove", string_list_to_python(py, &patch.remove)?)?;
        }
        EventCompiled::ExtCall { command, args } => {
            dict.set_item("type", "ext_call")?;
            dict.set_item("command", command)?;
            let list = PyList::empty(py);
            for arg in args {
                list.append(arg)?;
            }
            dict.set_item("args", list)?;
        }
        EventCompiled::AudioAction(action) => {
            dict.set_item("type", "audio_action")?;
            dict.set_item("channel", action.channel)?;
            dict.set_item("action", action.action)?;
            dict.set_item("asset", action.asset.as_deref())?;
            dict.set_item("volume", action.volume)?;
            dict.set_item("fade_duration_ms", action.fade_duration_ms)?;
            dict.set_item("loop_playback", action.loop_playback)?;
        }
        EventCompiled::Transition(trans) => {
            dict.set_item("type", "transition")?;
            dict.set_item("kind", trans.kind)?;
            dict.set_item("duration_ms", trans.duration_ms)?;
            dict.set_item("color", trans.color.as_deref())?;
        }
        EventCompiled::SetCharacterPosition(pos) => {
            dict.set_item("type", "set_character_position")?;
            dict.set_item("name", pos.name.as_ref())?;
            dict.set_item("x", pos.x)?;
            dict.set_item("y", pos.y)?;
            dict.set_item("scale", pos.scale)?;
        }
    }
    Ok(dict.into())
}

pub fn characters_to_python(
    py: Python<'_>,
    characters: &[CharacterPlacementCompiled],
) -> PyResult<PyObject> {
    let list = PyList::empty(py);
    for character in characters {
        let character_dict = PyDict::new(py);
        character_dict.set_item("name", character.name.as_ref())?;
        character_dict.set_item("expression", character.expression.as_deref())?;
        character_dict.set_item("position", character.position.as_deref())?;
        character_dict.set_item("x", character.x)?;
        character_dict.set_item("y", character.y)?;
        character_dict.set_item("scale", character.scale)?;
        list.append(character_dict)?;
    }
    Ok(list.into())
}

pub fn patch_update_to_python(
    py: Python<'_>,
    update: &[CharacterPatchCompiled],
) -> PyResult<PyObject> {
    let list = PyList::empty(py);
    for character in update {
        let character_dict = PyDict::new(py);
        character_dict.set_item("name", character.name.as_ref())?;
        character_dict.set_item("expression", character.expression.as_deref())?;
        character_dict.set_item("position", character.position.as_deref())?;
        list.append(character_dict)?;
    }
    Ok(list.into())
}

pub fn string_list_to_python(py: Python<'_>, items: &[SharedStr]) -> PyResult<PyObject> {
    let list = PyList::empty(py);
    for item in items {
        list.append(item.as_ref())?;
    }
    Ok(list.into())
}

pub fn ui_state_to_python(ui: &UiState, py: Python<'_>) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    match &ui.view {
        UiView::Dialogue { speaker, text } => {
            dict.set_item("type", "dialogue")?;
            dict.set_item("speaker", speaker)?;
            dict.set_item("text", text)?;
        }
        UiView::Choice { prompt, options } => {
            dict.set_item("type", "choice")?;
            dict.set_item("prompt", prompt)?;
            let list = PyList::empty(py);
            for option in options {
                list.append(option)?;
            }
            dict.set_item("options", list)?;
        }
        UiView::Scene { description } => {
            dict.set_item("type", "scene")?;
            dict.set_item("description", description)?;
        }
        UiView::System { message } => {
            dict.set_item("type", "system")?;
            dict.set_item("message", message)?;
        }
    }
    Ok(dict.into())
}
