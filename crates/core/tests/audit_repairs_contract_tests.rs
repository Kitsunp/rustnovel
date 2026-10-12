use std::collections::BTreeMap;

use visual_novel_engine::{
    analyze_flow_graph,
    runtime::{CmpOp, CondCompiled, DialogueCompiled, EventCompiled, ScriptCompiled, SharedStr},
    CharacterData, EntityKind, ResourceLimiter, SceneState, SecurityPolicy,
};

fn dialogue() -> EventCompiled {
    EventCompiled::Dialogue(DialogueCompiled {
        speaker: SharedStr::from("Narrator"),
        text: SharedStr::from("End"),
    })
}

#[cfg(any(feature = "python", feature = "python-embed"))]
#[test]
fn rep_python_event_object_and_runtime_dict_preserve_compiled_data() {
    use pyo3::prelude::*;
    use visual_novel_engine::runtime::{
        AudioActionCompiled, CharacterPatchCompiled, ScenePatchCompiled, SceneTransitionCompiled,
    };

    let events = [
        EventCompiled::SetVar {
            var_id: 3,
            value: -17,
        },
        EventCompiled::JumpIf {
            cond: CondCompiled::VarCmp {
                var_id: 3,
                op: CmpOp::Ge,
                value: 4,
            },
            target_ip: 8,
        },
        EventCompiled::Patch(ScenePatchCompiled {
            update: vec![CharacterPatchCompiled {
                name: "Ava".into(),
                x: Some(12),
                y: Some(34),
                scale: Some(1.25),
                ..Default::default()
            }],
            ..Default::default()
        }),
        EventCompiled::AudioAction(AudioActionCompiled {
            channel: 2,
            action: 2,
            asset: None,
            volume: Some(0.5),
            fade_duration_ms: Some(125),
            loop_playback: None,
        }),
        EventCompiled::Transition(SceneTransitionCompiled {
            kind: 2,
            duration_ms: 250,
            color: None,
        }),
    ];
    Python::with_gil(|py| {
        for event in events {
            let object = event.to_python(py).unwrap();
            let from_object = object.call_method0(py, "to_dict").unwrap();
            let from_runtime = event.to_python_dict(py).unwrap();
            assert!(from_object.bind(py).eq(from_runtime.bind(py)).unwrap());
            let dict = from_object.bind(py);
            for key in [
                "key",
                "value",
                "target",
                "target_ip",
                "update",
                "channel",
                "action",
                "kind",
                "channel_name",
                "action_name",
                "kind_name",
            ] {
                if let Ok(value) = dict.get_item(key) {
                    assert!(object.getattr(py, key).unwrap().bind(py).eq(value).unwrap());
                }
            }
            match event {
                EventCompiled::SetVar { .. } => {
                    assert_eq!(dict.get_item("key").unwrap().extract::<u32>().unwrap(), 3);
                    assert_eq!(
                        dict.get_item("value").unwrap().extract::<i32>().unwrap(),
                        -17
                    );
                }
                EventCompiled::JumpIf { .. } => {
                    assert_eq!(
                        dict.get_item("target").unwrap().extract::<u32>().unwrap(),
                        8
                    );
                    let cond = dict.get_item("cond").unwrap();
                    assert_eq!(
                        cond.get_item("kind").unwrap().extract::<String>().unwrap(),
                        "var_cmp"
                    );
                    assert_eq!(
                        cond.get_item("op").unwrap().extract::<String>().unwrap(),
                        "ge"
                    );
                }
                EventCompiled::Patch(_) => {
                    assert!(dict.get_item("background").is_err());
                    assert!(dict.get_item("music").is_err());
                    let update = dict.get_item("update").unwrap().get_item(0).unwrap();
                    assert_eq!(update.get_item("x").unwrap().extract::<i32>().unwrap(), 12);
                    assert_eq!(
                        update.get_item("scale").unwrap().extract::<f32>().unwrap(),
                        1.25
                    );
                }
                EventCompiled::AudioAction(_) => {
                    assert_eq!(
                        dict.get_item("channel").unwrap().extract::<u8>().unwrap(),
                        2
                    );
                    assert_eq!(
                        dict.get_item("channel_name")
                            .unwrap()
                            .extract::<String>()
                            .unwrap(),
                        "voice"
                    );
                    assert_eq!(
                        dict.get_item("action_name")
                            .unwrap()
                            .extract::<String>()
                            .unwrap(),
                        "fade_out"
                    );
                }
                EventCompiled::Transition(_) => {
                    assert_eq!(dict.get_item("kind").unwrap().extract::<u8>().unwrap(), 2);
                    assert_eq!(
                        dict.get_item("kind_name")
                            .unwrap()
                            .extract::<String>()
                            .unwrap(),
                        "cut"
                    );
                }
                _ => unreachable!(),
            }
        }
    });
}

#[cfg(any(feature = "python", feature = "python-embed"))]
#[test]
fn rep_python_raw_events_keep_all_fields_and_collect_cached_cycles() {
    use pyo3::{ffi::c_str, prelude::*, types::PyDict};
    use serde_json::json;
    use visual_novel_engine::runtime::EventRaw;

    let events = [
        json!({"type":"dialogue", "speaker":"Ava", "text":"Hello"}),
        json!({"type":"choice", "prompt":"Next?", "options":[{"text":"Go", "target":"end"}]}),
        json!({"type":"scene", "characters":[{"name":"Ava", "x":12, "y":34, "scale":1.25}]}),
        json!({"type":"jump", "target":"end"}),
        json!({"type":"set_flag", "key":"ready", "value":true}),
        json!({"type":"set_var", "key":"score", "value":-17}),
        json!({"type":"jump_if", "cond":{"kind":"var_cmp", "key":"score", "op":"ge", "value":4}, "target":"end"}),
        json!({"type":"patch", "update":[{"name":"Ava", "x":12, "y":34, "scale":1.25}]}),
        json!({"type":"patch", "background":null, "music":null}),
        json!({"type":"patch", "background":"bg/room.png", "music":"music/theme.ogg"}),
        json!({"type":"ext_call", "command":"notify", "args":["hello"]}),
        json!({"type":"audio_action", "channel":"sfx", "action":"play", "asset":"bell.wav", "volume":0.5, "fade_duration_ms":18446744073709551615u64, "loop_playback":false}),
        json!({"type":"transition", "kind":"dissolve", "duration_ms":250, "color":"#000000"}),
        json!({"type":"set_character_position", "name":"Ava", "x":12, "y":34, "scale":1.25}),
    ];
    Python::with_gil(|py| {
        let json_module = py.import("json").unwrap();
        for input in events {
            let event: EventRaw = serde_json::from_value(input).unwrap();
            assert_eq!(
                serde_json::from_value::<EventRaw>(event.to_json_value()).unwrap(),
                event,
                "raw serialization must preserve patch no-op fields"
            );
            if let EventRaw::Patch(patch) = &event {
                let bytes = postcard::to_allocvec(patch).unwrap();
                assert_eq!(
                    postcard::from_bytes::<visual_novel_engine::runtime::ScenePatchRaw>(&bytes)
                        .unwrap(),
                    *patch
                );
            }
            let object = event.to_python(py).unwrap();
            let dict = object.call_method0(py, "to_dict").unwrap();
            let encoded: String = json_module
                .call_method1("dumps", (dict.bind(py),))
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&encoded).unwrap(),
                event.to_json_value()
            );
            for key in event.to_json_value().as_object().unwrap().keys() {
                let value = dict.bind(py).get_item(key).unwrap();
                assert!(object
                    .getattr(py, key.as_str())
                    .unwrap()
                    .bind(py)
                    .eq(&value)
                    .unwrap());
                if ["options", "characters", "add", "update", "remove"].contains(&key.as_str()) {
                    assert!(object
                        .getattr(py, key.as_str())
                        .unwrap()
                        .bind(py)
                        .is(&value));
                }
            }
            assert!(object.getattr(py, "unknown_field").is_err());
        }
        let globals = PyDict::new(py);
        globals
            .set_item(
                "event",
                EventRaw::SetVar {
                    key: "score".into(),
                    value: 1,
                }
                .to_python(py)
                .unwrap(),
            )
            .unwrap();
        py.run(c_str!("import gc, weakref\nclass Marker: pass\nmarker = Marker()\nreference = weakref.ref(marker)\nevent.to_dict()['cycle'] = (event, marker)\ndel event, marker\ngc.collect()\nassert reference() is None"), Some(&globals), None).unwrap();
    });
}

#[test]
fn rep_prefetch_max_depth_is_bounded_after_advancing() {
    use visual_novel_engine::runtime::{DialogueRaw, Engine, EventRaw, SceneUpdateRaw, ScriptRaw};
    let script = ScriptRaw::new(
        vec![
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".into(),
                text: "Hello".into(),
            }),
            EventRaw::Scene(SceneUpdateRaw {
                background: Some("bg/room.png".into()),
                ..Default::default()
            }),
        ],
        BTreeMap::from([("start".into(), 0)]),
    );
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    engine.step().unwrap();
    assert!(engine.state().position > 0);
    assert_eq!(
        engine.peek_next_asset_paths(usize::MAX),
        vec!["bg/room.png"]
    );
}

#[test]
fn rep_branch_undo_restores_graph_and_history() {
    use visual_novel_engine::authoring::{
        authoring_graph_sha256, AuthoringCommand, AuthoringCommandBus, AuthoringPosition,
        NodeGraph, StoryNode,
    };
    let mut graph = NodeGraph::new();
    let from = graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let to = graph.add_node(StoryNode::End, AuthoringPosition::new(0.0, 100.0));
    let fingerprint = authoring_graph_sha256(&graph);
    let mut bus = AuthoringCommandBus::new(graph);
    bus.apply(AuthoringCommand::ConnectOrBranch {
        from,
        from_port: 0,
        to,
        branch_position: AuthoringPosition::new(100.0, 50.0),
    })
    .unwrap();
    bus.apply(AuthoringCommand::RevertLast)
        .expect("undo connection");
    assert_eq!(bus.undo_delta_count(), 0);
    assert_eq!(bus.redo_delta_count(), 1);
    assert_eq!(bus.operation_log().len(), 2);
    for _ in 0..2 {
        assert!(bus.apply(AuthoringCommand::RevertLast).is_err());
        assert_eq!(bus.undo_delta_count(), 0);
        assert_eq!(bus.redo_delta_count(), 1);
        assert_eq!(bus.operation_log().len(), 2);
        assert_eq!(authoring_graph_sha256(bus.graph()), fingerprint);
    }
}

fn compiled(events: Vec<EventCompiled>) -> ScriptCompiled {
    ScriptCompiled {
        events,
        labels: BTreeMap::from([("start".to_string(), 0)]),
        start_ip: 0,
        flag_count: 0,
    }
}

#[test]
fn rep_scene_deserialization_restores_lookups_and_unique_ids() {
    let mut scene = SceneState::new();
    let initial = scene
        .spawn(EntityKind::Character(CharacterData {
            name: "Mira".into(),
            expression: None,
        }))
        .expect("spawn");
    let bytes = serde_json::to_vec(&scene).expect("serialize");
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(json["entities"][0]["kind"]["type"], "character");
    let mut restored: SceneState = serde_json::from_slice(&bytes).expect("deserialize");
    assert!(
        restored.get(initial).is_some(),
        "index must be rebuilt by Deserialize"
    );
    let next = restored
        .spawn(EntityKind::Character(CharacterData {
            name: "Nara".into(),
            expression: None,
        }))
        .expect("spawn after deserialize");
    assert_ne!(next, initial, "deserialized next_id must be monotonic");
    assert!(restored.get(initial).is_some());
    assert!(restored.get(next).is_some());
}

#[test]
fn rep_scene_deserialization_rejects_duplicate_and_exhausted_ids() {
    let mut scene = SceneState::new();
    scene
        .spawn(EntityKind::Character(CharacterData {
            name: "Mira".into(),
            expression: None,
        }))
        .expect("spawn");
    let mut value = serde_json::to_value(&scene).expect("serialize");
    let first = value["entities"][0].clone();
    value["entities"]
        .as_array_mut()
        .expect("entities")
        .push(first);
    assert!(serde_json::from_value::<SceneState>(value).is_err());

    let mut value = serde_json::to_value(&scene).expect("serialize");
    value["entities"][0]["id"] = serde_json::json!(u32::MAX);
    assert!(serde_json::from_value::<SceneState>(value).is_err());
}

#[test]
fn rep_graph_cycles_do_not_contain_phantom_nodes() {
    let analysis = analyze_flow_graph(&[0, 1], &[(0, 1), (1, 99), (99, 1)], &[0]);
    assert!(analysis.reachable.contains(&1));
    assert!(!analysis.reachable.contains(&99));
    assert!(!analysis.reachable_cycle_nodes.contains(&99));
    assert!(analysis.reachable_cycle_nodes.is_empty());
}

#[test]
fn rep_compiled_var_id_allocation_is_bounded() {
    let bad = compiled(vec![EventCompiled::SetVar {
        var_id: u32::MAX,
        value: 1,
    }]);
    assert!(SecurityPolicy::default()
        .validate_compiled(&bad, ResourceLimiter::default())
        .is_err());
    let bad_cond = compiled(vec![EventCompiled::JumpIf {
        target_ip: 1,
        cond: CondCompiled::VarCmp {
            var_id: 999_999,
            op: CmpOp::Eq,
            value: 0,
        },
    }]);
    assert!(SecurityPolicy::default()
        .validate_compiled(&bad_cond, ResourceLimiter::default())
        .is_err());
}

#[test]
fn rep_eof_is_a_legitimate_terminal_ip_not_a_memory_bug() {
    let script = compiled(vec![EventCompiled::Jump { target_ip: 1 }]);
    SecurityPolicy::default()
        .validate_compiled(&script, ResourceLimiter::default())
        .expect("EOF must remain a legal explicit terminal");
    let bad = compiled(vec![EventCompiled::Jump { target_ip: 2 }]);
    assert!(SecurityPolicy::default()
        .validate_compiled(&bad, ResourceLimiter::default())
        .is_err());

    let mut script = compiled(vec![dialogue()]);
    script.labels.insert("bad".to_string(), u32::MAX);
    assert!(SecurityPolicy::default()
        .validate_compiled(&script, ResourceLimiter::default())
        .is_err());
}

#[test]
fn rep_deserialized_track_rejects_duplicate_and_unsorted_keyframes() {
    use visual_novel_engine::{Easing, EntityId, Keyframe, PropertyType, Track};
    let mut track = Track::new(EntityId::new(0), PropertyType::PositionX);
    track
        .add_keyframe(Keyframe::new(0, 0, Easing::Linear))
        .expect("first");
    track
        .add_keyframe(Keyframe::new(10, 50, Easing::Linear))
        .expect("second");
    let mut value = serde_json::to_value(&track).expect("serialize");
    value["keyframes"][1]["time"] = serde_json::json!(0);
    assert!(serde_json::from_value::<Track>(value).is_err());
    let mut value = serde_json::to_value(&track).expect("serialize");
    value["keyframes"][1]["time"] = serde_json::json!(u32::MAX);
    assert!(serde_json::from_value::<Track>(value).is_ok());
}

#[test]
fn rep_renpy_menu_branches_cannot_fall_through() {
    use visual_novel_engine::{
        import_renpy_project,
        runtime::{EventRaw, ScriptRaw},
        ImportFallbackPolicy, ImportProfile, ImportRenpyOptions,
    };
    let directory = tempfile::tempdir().expect("tempdir");
    let project_root = directory.path().join("project");
    let game = project_root.join("game");
    std::fs::create_dir_all(&game).expect("mkdir");
    std::fs::write(
        game.join("script.rpy"),
        r#"
label start:
    menu:
        "A":
            "Narrator" "Selected A"
        "B":
            "Narrator" "Selected B"
    "Narrator" "After menu"
"#,
    )
    .expect("write fixture");
    let output_root = directory.path().join("output");
    import_renpy_project(ImportRenpyOptions {
        project_root,
        output_root: output_root.clone(),
        entry_label: "start".to_string(),
        report_path: None,
        profile: ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: None,
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        strict_mode: false,
        fallback_policy: ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import");
    let raw = std::fs::read_to_string(output_root.join("main.json")).expect("read");
    let script = ScriptRaw::from_json(&raw).expect("parse");
    script.compile().expect("compiled menu must be valid");
    let choice = script
        .events
        .iter()
        .find_map(|event| {
            if let EventRaw::Choice(choice) = event {
                Some(choice)
            } else {
                None
            }
        })
        .expect("menu choice");
    assert_eq!(choice.options.len(), 2);
    let first = script.labels[&choice.options[0].target];
    let second = script.labels[&choice.options[1].target];
    assert!(first < second);
    assert!(
        script.events[first..second].iter().any(|event| {
            matches!(event, EventRaw::Jump { target } if target.contains("menu_end"))
        }),
        "first branch must explicitly jump past the second branch"
    );
}

#[test]
fn rep_scene_postcard_roundtrip_rebuilds_transient_index() {
    let mut scene = SceneState::new();
    let id = scene
        .spawn(EntityKind::Character(CharacterData {
            name: "Mira".into(),
            expression: None,
        }))
        .expect("spawn");
    let serialized = postcard::to_allocvec(&scene).expect("postcard encode");
    let mut decoded: SceneState = postcard::from_bytes(&serialized).expect("postcard decode");
    assert!(decoded.get(id).is_some());
    assert_ne!(
        decoded
            .spawn(EntityKind::Character(CharacterData {
                name: "Nara".into(),
                expression: None,
            }))
            .expect("spawn"),
        id
    );
}

#[test]
fn rep_renpy_duplicate_label_cannot_silently_overwrite_first() {
    use visual_novel_engine::{
        import_renpy_project, ImportFallbackPolicy, ImportProfile, ImportRenpyOptions,
    };
    let directory = tempfile::tempdir().expect("tempdir");
    let project_root = directory.path().join("project");
    let game = project_root.join("game");
    std::fs::create_dir_all(&game).expect("mkdir");
    std::fs::write(
        game.join("script.rpy"),
        "label start:\n    \"Hello\"\nlabel start:\n    \"Duplicate\"\n",
    )
    .expect("fixture");
    let result = import_renpy_project(ImportRenpyOptions {
        project_root,
        output_root: directory.path().join("output"),
        entry_label: "start".into(),
        report_path: None,
        profile: ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: None,
        include_patterns: vec![],
        exclude_patterns: vec![],
        strict_mode: false,
        fallback_policy: ImportFallbackPolicy::DegradeWithTrace,
    });
    assert!(
        result.is_err(),
        "duplicate source labels must reject import"
    );
}
