use super::python_bridge_helpers::{
    scene_compiled_to_python, scene_patch_add_compiled_to_python,
    scene_patch_remove_compiled_to_python, scene_patch_update_compiled_to_python,
};
use super::{
    audio_action_name, audio_channel_name, transition_kind_name, CmpOp, CondCompiled, EventCompiled,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};

pub(super) fn compiled_event_to_python_dict(
    event: &EventCompiled,
    py: Python<'_>,
) -> PyResult<Py<PyAny>> {
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
            dict.set_item("characters", scene_compiled_to_python(py, scene)?)?;
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
            dict.set_item("key", *var_id)?;
            dict.set_item("var_id", *var_id)?;
            dict.set_item("value", *value)?;
        }
        EventCompiled::JumpIf { cond, target_ip } => {
            dict.set_item("type", "jump_if")?;
            dict.set_item("cond", condition_to_python(cond, py)?)?;
            dict.set_item("target", *target_ip)?;
            dict.set_item("target_ip", *target_ip)?;
        }
        EventCompiled::Patch(patch) => {
            dict.set_item("type", "patch")?;
            if let Some(background) = &patch.background {
                dict.set_item("background", background.as_ref())?;
            }
            if let Some(music) = &patch.music {
                dict.set_item("music", music.as_ref())?;
            }
            dict.set_item("add", scene_patch_add_compiled_to_python(py, patch)?)?;
            dict.set_item("update", scene_patch_update_compiled_to_python(py, patch)?)?;
            dict.set_item("remove", scene_patch_remove_compiled_to_python(py, patch)?)?;
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
            dict.set_item("channel_name", audio_channel_name(action.channel))?;
            dict.set_item("action_name", audio_action_name(action.action))?;
            dict.set_item("asset", action.asset.as_deref())?;
            dict.set_item("volume", action.volume)?;
            dict.set_item("fade_duration_ms", action.fade_duration_ms)?;
            dict.set_item("loop_playback", action.loop_playback)?;
        }
        EventCompiled::Transition(trans) => {
            dict.set_item("type", "transition")?;
            dict.set_item("kind", trans.kind)?;
            dict.set_item("kind_name", transition_kind_name(trans.kind))?;
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

fn condition_to_python(cond: &CondCompiled, py: Python<'_>) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    match cond {
        CondCompiled::Flag { flag_id, is_set } => {
            dict.set_item("kind", "flag")?;
            dict.set_item("key", *flag_id)?;
            dict.set_item("flag_id", *flag_id)?;
            dict.set_item("is_set", *is_set)?;
        }
        CondCompiled::VarCmp { var_id, op, value } => {
            dict.set_item("kind", "var_cmp")?;
            dict.set_item("key", *var_id)?;
            dict.set_item("var_id", *var_id)?;
            dict.set_item(
                "op",
                match op {
                    CmpOp::Eq => "eq",
                    CmpOp::Ne => "ne",
                    CmpOp::Lt => "lt",
                    CmpOp::Le => "le",
                    CmpOp::Gt => "gt",
                    CmpOp::Ge => "ge",
                },
            )?;
            dict.set_item("value", *value)?;
        }
    }
    Ok(dict.into())
}
