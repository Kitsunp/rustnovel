use super::{ScenePatchCompiled, ScenePatchRaw, SceneUpdateCompiled, SceneUpdateRaw};

pub fn scene_to_python(
    py: pyo3::Python<'_>,
    scene: &SceneUpdateRaw,
) -> pyo3::PyResult<pyo3::PyObject> {
    use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
    let characters = PyList::empty(py);
    for character in &scene.characters {
        let character_dict = PyDict::new(py);
        character_dict.set_item("name", character.name.as_str())?;
        character_dict.set_item("expression", character.expression.as_deref())?;
        character_dict.set_item("position", character.position.as_deref())?;
        characters.append(character_dict)?;
    }
    Ok(characters.into())
}

pub fn scene_compiled_to_python(
    py: pyo3::Python<'_>,
    scene: &SceneUpdateCompiled,
) -> pyo3::PyResult<pyo3::PyObject> {
    use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
    let characters = PyList::empty(py);
    for character in &scene.characters {
        let character_dict = PyDict::new(py);
        character_dict.set_item("name", character.name.as_ref())?;
        character_dict.set_item("expression", character.expression.as_deref())?;
        character_dict.set_item("position", character.position.as_deref())?;
        characters.append(character_dict)?;
    }
    Ok(characters.into())
}

pub fn scene_patch_add_to_python(
    py: pyo3::Python<'_>,
    patch: &ScenePatchRaw,
) -> pyo3::PyResult<pyo3::PyObject> {
    use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
    let characters = PyList::empty(py);
    for character in &patch.add {
        let character_dict = PyDict::new(py);
        character_dict.set_item("name", character.name.as_str())?;
        character_dict.set_item("expression", character.expression.as_deref())?;
        character_dict.set_item("position", character.position.as_deref())?;
        characters.append(character_dict)?;
    }
    Ok(characters.into())
}

pub fn scene_patch_add_compiled_to_python(
    py: pyo3::Python<'_>,
    patch: &ScenePatchCompiled,
) -> pyo3::PyResult<pyo3::PyObject> {
    use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
    let characters = PyList::empty(py);
    for character in &patch.add {
        let character_dict = PyDict::new(py);
        character_dict.set_item("name", character.name.as_ref())?;
        character_dict.set_item("expression", character.expression.as_deref())?;
        character_dict.set_item("position", character.position.as_deref())?;
        characters.append(character_dict)?;
    }
    Ok(characters.into())
}

pub fn scene_patch_update_to_python(
    py: pyo3::Python<'_>,
    patch: &ScenePatchRaw,
) -> pyo3::PyResult<pyo3::PyObject> {
    use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
    let characters = PyList::empty(py);
    for character in &patch.update {
        let character_dict = PyDict::new(py);
        character_dict.set_item("name", character.name.as_str())?;
        character_dict.set_item("expression", character.expression.as_deref())?;
        character_dict.set_item("position", character.position.as_deref())?;
        characters.append(character_dict)?;
    }
    Ok(characters.into())
}

pub fn scene_patch_update_compiled_to_python(
    py: pyo3::Python<'_>,
    patch: &ScenePatchCompiled,
) -> pyo3::PyResult<pyo3::PyObject> {
    use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
    let characters = PyList::empty(py);
    for character in &patch.update {
        let character_dict = PyDict::new(py);
        character_dict.set_item("name", character.name.as_ref())?;
        character_dict.set_item("expression", character.expression.as_deref())?;
        character_dict.set_item("position", character.position.as_deref())?;
        characters.append(character_dict)?;
    }
    Ok(characters.into())
}

pub fn scene_patch_remove_to_python(
    py: pyo3::Python<'_>,
    patch: &ScenePatchRaw,
) -> pyo3::PyResult<pyo3::PyObject> {
    use pyo3::types::{PyList, PyListMethods};
    let list = PyList::empty(py);
    for name in &patch.remove {
        list.append(name.as_str())?;
    }
    Ok(list.into())
}

pub fn scene_patch_remove_compiled_to_python(
    py: pyo3::Python<'_>,
    patch: &ScenePatchCompiled,
) -> pyo3::PyResult<pyo3::PyObject> {
    use pyo3::types::{PyList, PyListMethods};
    let list = PyList::empty(py);
    for name in &patch.remove {
        list.append(name.as_ref())?;
    }
    Ok(list.into())
}
