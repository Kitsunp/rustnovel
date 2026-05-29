use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use tempfile::TempDir;

use visual_novel_engine::{
    build_route_tree, export_bundle,
    runtime::{
        CharacterPlacementCompiled, ChoiceOptionRaw, ChoiceRaw, DialogueRaw, Engine, EventRaw,
        SceneUpdateCompiled, SceneUpdateRaw, ScriptRaw, VisualState,
    },
    BundleIntegrity, ExportBundleSpec, ExportTargetPlatform, LruCache, ProjectManifest,
    ResourceLimiter, SecurityPolicy,
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

fn bench_parse_large_json_matrix(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_large_json_matrix");
    for size in [10_000usize, 50_000, 100_000] {
        let raw = loop_script(size);
        let mut value = serde_json::to_value(&raw).expect("value");
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "script_schema_version".to_string(),
                serde_json::Value::String(visual_novel_engine::SCRIPT_SCHEMA_VERSION.to_string()),
            );
        }
        let json = serde_json::to_string(&value).expect("json");
        let limits = ResourceLimiter {
            max_events: size + 1,
            max_script_bytes: json.len() + 1024,
            ..ResourceLimiter::default()
        };
        group.bench_function(format!("events_{size}"), |b| {
            b.iter(|| ScriptRaw::from_json_with_limits(&json, limits).expect("parse large"))
        });
    }
    group.finish();
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
                    engine.step().expect("step");
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

fn bench_route_tree(c: &mut Criterion) {
    let compiled = loop_script(1_000).compile().expect("compile");
    c.bench_function("route_tree_1000_events", |b| {
        b.iter(|| build_route_tree(&compiled))
    });
}

fn bench_scene_frame(c: &mut Criterion) {
    let raw = sample_raw_script();
    let policy = SecurityPolicy::default();
    let limits = ResourceLimiter::default();
    c.bench_function("scene_frame_snapshot", |b| {
        b.iter_batched(
            || Engine::new(raw.clone(), policy.clone(), limits).expect("engine"),
            |engine| engine.scene_frame(),
            BatchSize::SmallInput,
        )
    });
}

fn bench_lru_cache_shared_hits(c: &mut Criterion) {
    c.bench_function("lru_cache_shared_hits_1mb", |b| {
        b.iter_batched(
            || {
                let mut cache = LruCache::<u32>::new(4 * 1024 * 1024);
                cache.insert(1, vec![7; 1024 * 1024]);
                cache
            },
            |mut cache| {
                for _ in 0..64 {
                    let bytes = cache.get(&1).expect("cache hit");
                    criterion::black_box(bytes);
                }
            },
            BatchSize::SmallInput,
        )
    });
}

fn build_export_bench_project(
    asset_count: usize,
    asset_size: usize,
) -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(root.join("assets")).expect("assets dir");
    ProjectManifest::new("bench", "bench")
        .save(&root.join("project.vnm"))
        .expect("manifest save");
    for index in 0..asset_count {
        fs::write(
            root.join("assets").join(format!("asset_{index}.bin")),
            vec![index as u8; asset_size],
        )
        .expect("asset write");
    }
    let mut events = Vec::with_capacity(asset_count + 1);
    for index in 0..asset_count {
        events.push(EventRaw::Scene(SceneUpdateRaw {
            background: Some(format!("assets/asset_{index}.bin")),
            music: None,
            characters: Vec::new(),
        }));
    }
    events.push(EventRaw::Dialogue(DialogueRaw {
        speaker: "Bench".to_string(),
        text: "done".to_string(),
    }));
    let script = ScriptRaw::new(events, BTreeMap::from([("start".to_string(), 0)]));
    fs::write(
        root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");
    (tmp, root)
}

fn bench_export_assets_streaming(c: &mut Criterion) {
    c.bench_function("export_assets_streaming_16x64kb", |b| {
        b.iter_batched(
            || build_export_bench_project(16, 64 * 1024),
            |(_tmp, root)| {
                let out = root.join("dist");
                let report = export_bundle(ExportBundleSpec {
                    project_root: root,
                    output_root: out,
                    target_platform: ExportTargetPlatform::Linux,
                    entry_script: None,
                    runtime_artifact: None,
                    integrity: BundleIntegrity::None,
                    output_layout_version: 1,
                    hmac_key: None,
                })
                .expect("export bench");
                criterion::black_box(report);
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    core_benches,
    bench_parse_json,
    bench_parse_large_json_matrix,
    bench_compile_script,
    bench_step_loop,
    bench_choice,
    bench_apply_scene,
    bench_route_tree,
    bench_scene_frame,
    bench_lru_cache_shared_hits,
    bench_export_assets_streaming
);
criterion_main!(core_benches);
