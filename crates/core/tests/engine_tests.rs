use std::collections::BTreeMap;

use visual_novel_engine::{
    runtime::{
        CharacterPatchRaw, CharacterPlacementRaw, Engine, EventCompiled, EventRaw, ScenePatchRaw,
        SceneUpdateRaw, ScriptRaw, SetCharacterPositionRaw,
    },
    RenderBackend, ResourceLimiter, SecurityPolicy, TextRenderer,
};

fn sample_script() -> ScriptRaw {
    let events = vec![
        EventRaw::Scene(SceneUpdateRaw {
            background: Some("bg/room.png".to_string()),
            music: Some("music/theme.ogg".to_string()),
            characters: vec![CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("smile".to_string()),
                position: Some("center".to_string()),
                x: None,
                y: None,
                scale: None,
            }],
        }),
        EventRaw::Dialogue(visual_novel_engine::runtime::DialogueRaw {
            speaker: "Ava".to_string(),
            text: "Hola".to_string(),
        }),
        EventRaw::Choice(visual_novel_engine::runtime::ChoiceRaw {
            prompt: "Ir?".to_string(),
            options: vec![
                visual_novel_engine::runtime::ChoiceOptionRaw {
                    text: "Si".to_string(),
                    target: "end".to_string(),
                },
                visual_novel_engine::runtime::ChoiceOptionRaw {
                    text: "No".to_string(),
                    target: "start".to_string(),
                },
            ],
        }),
        EventRaw::Dialogue(visual_novel_engine::runtime::DialogueRaw {
            speaker: "Ava".to_string(),
            text: "Fin".to_string(),
        }),
    ];
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    labels.insert("end".to_string(), 3);
    ScriptRaw::new(events, labels)
}

fn script_without_start_label() -> ScriptRaw {
    let events = vec![EventRaw::Dialogue(
        visual_novel_engine::runtime::DialogueRaw {
            speaker: "Ava".to_string(),
            text: "Hola".to_string(),
        },
    )];
    let labels = BTreeMap::new();
    ScriptRaw::new(events, labels)
}

fn script_with_invalid_choice_target() -> ScriptRaw {
    let events = vec![EventRaw::Choice(visual_novel_engine::runtime::ChoiceRaw {
        prompt: "Ir?".to_string(),
        options: vec![visual_novel_engine::runtime::ChoiceOptionRaw {
            text: "Si".to_string(),
            target: "missing".to_string(),
        }],
    })];
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    ScriptRaw::new(events, labels)
}

#[path = "engine_tests/compile_parity.rs"]
mod compile_parity;
#[path = "engine_tests/runtime_flow.rs"]
mod runtime_flow;
#[path = "engine_tests/visual_state.rs"]
mod visual_state;
