use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use visual_novel_engine::{
    CharacterPatchRaw, CharacterPlacementRaw, CondRaw, EventRaw, ScenePatchRaw,
};
use visual_novel_gui::editor::StoryNode;

use super::support::parse_cmp_op;

/// A node in the story graph.
#[pyclass(name = "StoryNode")]
#[derive(Clone)]
pub struct PyStoryNode {
    pub(super) inner: StoryNode,
}

impl PyStoryNode {
    pub(super) fn into_inner(self) -> StoryNode {
        self.inner
    }
}

#[pymethods]
impl PyStoryNode {
    #[staticmethod]
    fn dialogue(speaker: String, text: String) -> Self {
        Self {
            inner: StoryNode::Dialogue { speaker, text },
        }
    }

    #[staticmethod]
    fn choice(prompt: String, options: Vec<String>) -> Self {
        Self {
            inner: StoryNode::Choice { prompt, options },
        }
    }

    #[staticmethod]
    #[pyo3(signature = (background=None, music=None, characters=Vec::new()))]
    fn scene(
        background: Option<String>,
        music: Option<String>,
        characters: Vec<(String, Option<String>, Option<String>)>,
    ) -> Self {
        let characters = characters
            .into_iter()
            .map(|(name, expression, position)| CharacterPlacementRaw {
                name,
                expression,
                position,
                x: None,
                y: None,
                scale: None,
            })
            .collect();

        Self {
            inner: StoryNode::Scene {
                profile: None,
                background,
                music,
                characters,
            },
        }
    }

    #[staticmethod]
    fn jump(target: String) -> Self {
        Self {
            inner: StoryNode::Jump { target },
        }
    }

    #[staticmethod]
    fn set_variable(key: String, value: i32) -> Self {
        Self {
            inner: StoryNode::SetVariable { key, value },
        }
    }

    #[staticmethod]
    fn jump_if_flag(key: String, is_set: bool, target: String) -> Self {
        Self {
            inner: StoryNode::JumpIf {
                target,
                cond: CondRaw::Flag { key, is_set },
            },
        }
    }

    #[staticmethod]
    fn jump_if_var(key: String, op: String, value: i32, target: String) -> PyResult<Self> {
        Ok(Self {
            inner: StoryNode::JumpIf {
                target,
                cond: CondRaw::VarCmp {
                    key,
                    op: parse_cmp_op(&op)?,
                    value,
                },
            },
        })
    }

    #[staticmethod]
    #[pyo3(signature = (background=None, music=None, add=Vec::new(), update=Vec::new(), remove=Vec::new()))]
    fn scene_patch(
        background: Option<String>,
        music: Option<String>,
        add: Vec<(String, Option<String>, Option<String>)>,
        update: Vec<(String, Option<String>, Option<String>)>,
        remove: Vec<String>,
    ) -> Self {
        let add = add
            .into_iter()
            .map(|(name, expression, position)| CharacterPlacementRaw {
                name,
                expression,
                position,
                x: None,
                y: None,
                scale: None,
            })
            .collect();
        let update = update
            .into_iter()
            .map(|(name, expression, position)| CharacterPatchRaw {
                name,
                expression,
                position,
            })
            .collect();

        Self {
            inner: StoryNode::ScenePatch(ScenePatchRaw {
                background,
                music,
                add,
                update,
                remove,
            }),
        }
    }

    #[staticmethod]
    #[pyo3(signature = (channel, action, asset=None, volume=None, fade_duration_ms=None, loop_playback=None))]
    fn audio_action(
        channel: String,
        action: String,
        asset: Option<String>,
        volume: Option<f32>,
        fade_duration_ms: Option<u64>,
        loop_playback: Option<bool>,
    ) -> Self {
        Self {
            inner: StoryNode::AudioAction {
                channel,
                action,
                asset,
                volume,
                fade_duration_ms,
                loop_playback,
            },
        }
    }

    #[staticmethod]
    #[pyo3(signature = (kind, duration_ms, color=None))]
    fn transition(kind: String, duration_ms: u32, color: Option<String>) -> Self {
        Self {
            inner: StoryNode::Transition {
                kind,
                duration_ms,
                color,
            },
        }
    }

    #[staticmethod]
    #[pyo3(signature = (name, x, y, scale=None))]
    fn character_placement(name: String, x: i32, y: i32, scale: Option<f32>) -> Self {
        Self {
            inner: StoryNode::CharacterPlacement { name, x, y, scale },
        }
    }

    #[staticmethod]
    fn generic(event_json: String) -> PyResult<Self> {
        let event: EventRaw = serde_json::from_str(&event_json)
            .map_err(|err| PyValueError::new_err(format!("Invalid event JSON: {err}")))?;
        Ok(Self {
            inner: StoryNode::Generic(event),
        })
    }

    #[staticmethod]
    fn start() -> Self {
        Self {
            inner: StoryNode::Start,
        }
    }

    #[staticmethod]
    fn end() -> Self {
        Self {
            inner: StoryNode::End,
        }
    }

    #[getter]
    fn node_type(&self) -> String {
        self.inner.type_name().to_string()
    }

    fn __repr__(&self) -> String {
        format!("StoryNode({})", self.inner.type_name())
    }
}

impl From<StoryNode> for PyStoryNode {
    fn from(inner: StoryNode) -> Self {
        Self { inner }
    }
}
