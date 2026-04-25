use std::collections::BTreeMap;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

use visual_novel_engine::{
    CharacterPlacementCompiled, ChoiceOptionRaw, ChoiceRaw, DialogueRaw, Engine, EventRaw,
    ResourceLimiter, SceneUpdateCompiled, SceneUpdateRaw, ScriptRaw, SecurityPolicy, VisualState,
};

fn sample_raw_script() -> ScriptRaw {
    let events = vec![
        EventRaw::Dialogue(DialogueRaw {
            speaker: "A".to_string(),
            text: "Hello there".to_string(),
        }),
        EventRaw::Choice(ChoiceRaw {
            prompt: "Pick one".to_string(),
            options: vec![
                ChoiceOptionRaw {
                    text: "Go".to_string(),
                    target: "next".to_string(),
                },
                ChoiceOptionRaw {
                    text: "Stay".to_string(),
                    target: "next".to_string(),
                },
            ],
        }),
        EventRaw::Scene(SceneUpdateRaw {
            background: Some("bg_room".to_string()),
            music: Some("song".to_string()),
            characters: vec![],
        }),
        EventRaw::Dialogue(DialogueRaw {
            speaker: "B".to_string(),
            text: "After choice".to_string(),
        }),
    ];
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    labels.insert("next".to_string(), 2);
    ScriptRaw { events, labels }
}

fn loop_script(event_count: usize) -> ScriptRaw {
    let events = (0..event_count)
        .map(|idx| {
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: format!("Line {idx}"),
            })
        })
        .collect();
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    ScriptRaw { events, labels }
}

fn choice_script() -> ScriptRaw {
    let events = vec![
        EventRaw::Choice(ChoiceRaw {
            prompt: "Pick".to_string(),
            options: vec![
                ChoiceOptionRaw {
                    text: "Yes".to_string(),
                    target: "next".to_string(),
                },
                ChoiceOptionRaw {
                    text: "No".to_string(),
                    target: "next".to_string(),
                },
            ],
        }),
        EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "Done".to_string(),
        }),
    ];
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    labels.insert("next".to_string(), 1);
    ScriptRaw { events, labels }
}

fn bench_parse_json(c: &mut Criterion) {
    let raw = sample_raw_script();
    let mut value = serde_json::to_value(&raw).expect("value");
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "script_schema_version".to_string(),
            serde_json::Value::String(visual_novel_engine::SCRIPT_SCHEMA_VERSION.to_string()),
        );
    }
    let json = serde_json::to_string(&value).expect("json");
    c.bench_function("parse_json_to_raw", |b| {
        b.iter(|| ScriptRaw::from_json(&json).expect("parse"))
    });
}

fn bench_compile_script(c: &mut Criterion) {
    let raw = sample_raw_script();
    c.bench_function("compile_to_compiled", |b| {
        b.iter(|| raw.compile().expect("compile"))
    });
}

fn bench_step_loop(c: &mut Criterion) {
    let raw = loop_script(200);
    let policy = SecurityPolicy::default();
    let limits = ResourceLimiter::default();
    c.bench_function("step_loop", |b| {
        b.iter_batched(
            || Engine::new(raw.clone(), policy.clone(), limits).expect("engine"),
            |mut engine| {
                for _ in 0..200 {
                    let _ = engine.step().expect("step");
                }
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_choice(c: &mut Criterion) {
    let raw = choice_script();
    let policy = SecurityPolicy::default();
    let limits = ResourceLimiter::default();
    c.bench_function("choose_option", |b| {
        b.iter_batched(
            || Engine::new(raw.clone(), policy.clone(), limits).expect("engine"),
            |mut engine| {
                engine.choose(0).expect("choose");
            },
            BatchSize::SmallInput,
        )
    });
}

fn build_scene_update(count: usize) -> SceneUpdateCompiled {
    let mut characters = Vec::with_capacity(count);
    for idx in 0..count {
        let name: Arc<str> = Arc::from(format!("Hero{idx}"));
        characters.push(CharacterPlacementCompiled {
            name,
            expression: Some(Arc::from("happy")),
            position: Some(Arc::from("center")),
            x: None,
            y: None,
            scale: None,
        });
    }
    SceneUpdateCompiled {
        background: Some(Arc::from("bg_scene")),
        music: Some(Arc::from("theme")),
        characters,
    }
}

fn bench_apply_scene(c: &mut Criterion) {
    let mut group = c.benchmark_group("apply_scene");
    for size in [0usize, 5, 20, 50] {
        let scene = build_scene_update(size);
        group.bench_function(format!("characters_{size}"), |b| {
            b.iter_batched(
                VisualState::default,
                |mut state| state.apply_scene(&scene),
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(
    core_benches,
    bench_parse_json,
    bench_compile_script,
    bench_step_loop,
    bench_choice,
    bench_apply_scene
);
criterion_main!(core_benches);
