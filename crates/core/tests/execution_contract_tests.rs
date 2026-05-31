use std::collections::{BTreeMap, BTreeSet};

use visual_novel_engine::{
    authoring::{AuthoringPosition, LintCode, NodeGraph, StoryNode},
    runtime::{
        contract_for_authoring_node, contract_for_event_raw, contract_matrix,
        event_asset_refs_for_raw, event_behavior_for_compiled, event_behavior_for_raw, event_spec,
        event_specs, node_asset_refs_for_authoring_node, node_behavior_for_authoring_node,
        node_from_event_raw, node_kind_for_authoring_node, node_kind_for_event_raw,
        node_quick_fixes_for_issue, node_spec_for_authoring_node, node_specs,
        node_to_event_raw_without_export_context, AudioActionRaw, BehaviorQuickFixRisk,
        BehaviorSupport, CharacterPlacementRaw, ChoiceOptionRaw, ChoiceRaw, CompileCtx,
        DialogueRaw, EngineState, EventBehavior, EventCompiled, EventFlow, EventKind, EventRaw,
        ExecutionCtx, FidelityClass, NodeBehavior, NodeKind, NodeToEventError, PortOutput,
        PreviewCtx, QuickFixCtx, RenderCommand, SceneFrameCtx, SceneUpdateRaw, ScriptRaw,
        ValidationCtx,
    },
};

#[test]
fn matrix_contains_preview_and_runtime_contracts() {
    assert!(contract_matrix()
        .iter()
        .any(|entry| entry.fidelity == FidelityClass::PreviewOnly));
    assert!(contract_matrix()
        .iter()
        .any(|entry| entry.fidelity == FidelityClass::RuntimeReal));
    assert!(contract_matrix()
        .iter()
        .any(|entry| entry.fidelity == FidelityClass::FallbackDegraded));
    assert!(contract_matrix()
        .iter()
        .any(|entry| entry.fidelity == FidelityClass::HostRequired));
}

#[test]
fn story_markers_are_preview_only() {
    let start = contract_for_authoring_node(&StoryNode::Start);
    let end = contract_for_authoring_node(&StoryNode::End);
    assert_eq!(start.fidelity, FidelityClass::PreviewOnly);
    assert_eq!(end.fidelity, FidelityClass::PreviewOnly);
}

#[test]
fn raw_dialogue_is_runtime_real() {
    let contract = contract_for_event_raw(&EventRaw::Dialogue(DialogueRaw {
        speaker: "A".to_string(),
        text: "B".to_string(),
    }));
    assert_eq!(contract.event_name, "Dialogue");
    assert_eq!(contract.fidelity, FidelityClass::RuntimeReal);
    assert!(contract.export_supported);
}

#[test]
fn extcall_generic_node_requires_host_capability_for_export() {
    let contract = contract_for_authoring_node(&StoryNode::Generic(EventRaw::ExtCall {
        command: "hook".to_string(),
        args: vec!["x".to_string()],
    }));
    assert!(contract.runtime_supported);
    assert!(!contract.export_supported);
    assert_eq!(contract.fidelity, FidelityClass::HostRequired);
}

#[test]
fn execution_contract_matrix_projects_node_specs() {
    assert_eq!(contract_matrix().len(), node_specs().len());
    for spec in node_specs() {
        let contract = contract_matrix()
            .iter()
            .find(|entry| entry.event_name == spec.contract_name)
            .unwrap_or_else(|| panic!("missing contract for {}", spec.contract_name));
        assert_eq!(
            contract.editor_supported,
            spec.capabilities.editor_supported
        );
        assert_eq!(
            contract.preview_supported,
            spec.capabilities.preview_supported
        );
        assert_eq!(
            contract.runtime_supported,
            spec.capabilities.runtime_supported
        );
        assert_eq!(
            contract.export_supported,
            spec.capabilities.export_supported
        );
        assert_eq!(contract.fidelity, spec.capabilities.fidelity);
    }
}

#[test]
fn event_specs_expose_stable_schema_trace_and_capabilities() {
    let stable_names = event_specs()
        .iter()
        .map(|spec| spec.stable_name)
        .collect::<Vec<_>>();
    assert_eq!(stable_names.as_slice(), EventKind::STABLE_NAMES);
    assert_eq!(EventRaw::TYPE_NAMES, EventKind::STABLE_NAMES);

    let dialogue = event_spec(EventKind::Dialogue);
    assert_eq!(dialogue.contract_name, "Dialogue");
    assert_eq!(dialogue.raw_schema, "EventRaw::Dialogue(DialogueRaw)");
    assert_eq!(
        dialogue.compiled_schema,
        "EventCompiled::Dialogue(DialogueCompiled)"
    );
    assert_eq!(dialogue.trace_kind, "dialogue");
    assert_eq!(dialogue.raw_field_paths, &["speaker", "text"]);

    let choice = event_spec(EventKind::Choice);
    assert_eq!(choice.flow, EventFlow::ChoiceTargets);
    assert_eq!(choice.capabilities.interactions, BehaviorSupport::Native);

    let scene = event_spec(EventKind::Scene);
    assert_eq!(scene.capabilities.visual_state, BehaviorSupport::Native);
    assert_eq!(scene.capabilities.asset_refs, BehaviorSupport::Native);
    assert_eq!(
        scene.asset_ref_field_paths,
        &["background", "music", "characters[].expression"]
    );

    let ext_call = event_spec(EventKind::ExtCall);
    assert_eq!(ext_call.capabilities.execute, BehaviorSupport::HostRequired);
    assert!(!ext_call.capabilities.headless_supported);
    assert!(!ext_call.capabilities.export_supported);
}

#[test]
fn event_behavior_compiles_raw_events_through_canonical_contract() {
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    labels.insert("next".to_string(), 1);

    let mut ctx = CompileCtx::new(&labels);
    let choice = EventRaw::Choice(ChoiceRaw {
        prompt: "Pick".to_string(),
        options: vec![ChoiceOptionRaw {
            text: "Continue".to_string(),
            target: "next".to_string(),
        }],
    });
    let compiled = event_behavior_for_raw(&choice)
        .compile(&mut ctx, &choice)
        .expect("choice compiles via behavior");
    assert!(matches!(
        compiled,
        EventCompiled::Choice(choice) if choice.options[0].target_ip == 1
    ));

    let set_flag = EventRaw::SetFlag {
        key: "seen_intro".to_string(),
        value: true,
    };
    let compiled = event_behavior_for_raw(&set_flag)
        .compile(&mut ctx, &set_flag)
        .expect("state event compiles via behavior");
    assert!(matches!(
        compiled,
        EventCompiled::SetFlag {
            flag_id: 0,
            value: true
        }
    ));
    assert_eq!(ctx.flag_count(), 1);
}

#[test]
fn event_behavior_executes_compiled_events_through_canonical_contract() {
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    let script = ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: Some("bg/room.png".to_string()),
                music: Some("music/theme.ogg".to_string()),
                characters: vec![],
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Ava".to_string(),
                text: "Hello".to_string(),
            }),
            EventRaw::SetFlag {
                key: "seen_intro".to_string(),
                value: true,
            },
        ],
        labels,
    )
    .compile()
    .expect("script compiles through behavior");

    let mut state = EngineState::new(script.start_ip, script.flag_count);
    let mut audio_commands = Vec::new();
    let mut read_dialogue_ips = BTreeSet::new();
    let mut route_visited_ips = BTreeSet::new();
    let mut pending_transition = None;

    {
        let event = &script.events[0];
        let mut ctx = ExecutionCtx::new(
            &mut state,
            &script.events,
            &mut audio_commands,
            &mut read_dialogue_ips,
            &mut route_visited_ips,
            &mut pending_transition,
        );
        event_behavior_for_compiled(event)
            .execute(&mut ctx, event)
            .expect("scene executes via behavior");
    }
    assert_eq!(state.position, 1);
    assert!(audio_commands.iter().any(|command| matches!(
        command,
        visual_novel_engine::runtime::AudioCommand::PlayBgm { path, .. }
            if path.as_ref() == "music/theme.ogg"
    )));

    {
        let event = &script.events[1];
        let mut ctx = ExecutionCtx::new(
            &mut state,
            &script.events,
            &mut audio_commands,
            &mut read_dialogue_ips,
            &mut route_visited_ips,
            &mut pending_transition,
        );
        event_behavior_for_compiled(event)
            .execute(&mut ctx, event)
            .expect("dialogue executes via behavior");
    }
    assert_eq!(state.position, 2);
    assert!(read_dialogue_ips.contains(&1));

    {
        let event = &script.events[2];
        let mut ctx = ExecutionCtx::new(
            &mut state,
            &script.events,
            &mut audio_commands,
            &mut read_dialogue_ips,
            &mut route_visited_ips,
            &mut pending_transition,
        );
        event_behavior_for_compiled(event)
            .execute(&mut ctx, event)
            .expect("state event executes via behavior");
    }
    assert_eq!(state.position, 3);
    assert!(state.get_flag(0));
    assert!(route_visited_ips.contains(&0));
    assert!(route_visited_ips.contains(&1));
    assert!(route_visited_ips.contains(&2));
}

#[test]
fn event_behavior_previews_visuals_and_scene_frame_interactions() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: Some("bg/room.png".to_string()),
                music: None,
                characters: vec![CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("sprites/ava.png".to_string()),
                    ..Default::default()
                }],
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Ava".to_string(),
                text: "Ready".to_string(),
            }),
            EventRaw::Choice(ChoiceRaw {
                prompt: "Where?".to_string(),
                options: vec![ChoiceOptionRaw {
                    text: "Left".to_string(),
                    target: "end".to_string(),
                }],
            }),
        ],
        BTreeMap::from([("start".to_string(), 0), ("end".to_string(), 3)]),
    )
    .compile()
    .expect("script compiles through behavior");

    let mut state = EngineState::new(script.start_ip, script.flag_count);
    {
        let scene = &script.events[0];
        let mut ctx = PreviewCtx::new(&mut state.visual);
        assert!(event_behavior_for_compiled(scene)
            .preview(&mut ctx, scene)
            .expect("scene previews via behavior")
            .is_none());
    }
    assert_eq!(state.visual.background.as_deref(), Some("bg/room.png"));
    assert_eq!(state.visual.characters.len(), 1);

    let mut commands = Vec::new();
    let mut interactions = Vec::new();
    {
        let dialogue = &script.events[1];
        let mut ctx = SceneFrameCtx::new(&mut commands, &mut interactions);
        event_behavior_for_compiled(dialogue)
            .append_scene_frame(&mut ctx, dialogue)
            .expect("dialogue frame comes from behavior");
    }
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Text { text, style, .. } if text == "Ready" && style == "dialogue.text"
    )));
    assert!(interactions
        .iter()
        .any(|interaction| interaction.action == "advance"));

    commands.clear();
    interactions.clear();
    {
        let choice = &script.events[2];
        let mut ctx = SceneFrameCtx::new(&mut commands, &mut interactions);
        event_behavior_for_compiled(choice)
            .append_scene_frame(&mut ctx, choice)
            .expect("choice frame comes from behavior");
    }
    assert!(commands
        .iter()
        .any(|command| matches!(command, RenderCommand::Button { label, .. } if label == "Left")));
    assert_eq!(
        interactions
            .iter()
            .map(|interaction| interaction.action.as_str())
            .collect::<Vec<_>>(),
        vec!["choose:0"]
    );
}

#[test]
fn node_behavior_validates_and_suggests_quick_fixes_through_contract() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: String::new(),
            text: "Hola".to_string(),
        },
        AuthoringPosition::new(0.0, 90.0),
    );
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Pick".to_string(),
            options: Vec::new(),
        },
        AuthoringPosition::new(0.0, 180.0),
    );
    let audio = graph.add_node(
        StoryNode::AudioAction {
            channel: "music".to_string(),
            action: "play".to_string(),
            asset: None,
            volume: None,
            fade_duration_ms: None,
            loop_playback: None,
        },
        AuthoringPosition::new(0.0, 270.0),
    );
    graph.connect(start, dialogue);
    graph.connect(dialogue, choice);

    let labels = BTreeSet::new();
    let asset_exists = |_asset: &str| true;
    let dialogue_node = graph.get_node(dialogue).expect("dialogue node");
    let validation_ctx = ValidationCtx::new(&graph, dialogue, &labels, &asset_exists);
    let dialogue_issues =
        node_behavior_for_authoring_node(dialogue_node).validate(&validation_ctx, dialogue_node);
    let speaker_issue = dialogue_issues
        .iter()
        .find(|issue| issue.code == LintCode::EmptySpeakerName)
        .expect("dialogue behavior reports empty speaker");
    let speaker_fixes = node_quick_fixes_for_issue(&QuickFixCtx::new(&graph), speaker_issue);
    assert_eq!(speaker_fixes[0].fix_id, "dialogue_fill_speaker");
    assert_eq!(speaker_fixes[0].risk, BehaviorQuickFixRisk::Safe);

    let choice_node = graph.get_node(choice).expect("choice node");
    let validation_ctx = ValidationCtx::new(&graph, choice, &labels, &asset_exists);
    let choice_issues =
        node_behavior_for_authoring_node(choice_node).validate(&validation_ctx, choice_node);
    let choice_issue = choice_issues
        .iter()
        .find(|issue| issue.code == LintCode::ChoiceNoOptions)
        .expect("choice behavior reports missing options");
    let choice_fixes = node_quick_fixes_for_issue(&QuickFixCtx::new(&graph), choice_issue);
    assert!(choice_fixes
        .iter()
        .any(|fix| fix.fix_id == "choice_add_default_option_to_end"));

    let audio_node = graph.get_node(audio).expect("audio node");
    let validation_ctx = ValidationCtx::new(&graph, audio, &labels, &asset_exists);
    let audio_issues =
        node_behavior_for_authoring_node(audio_node).validate(&validation_ctx, audio_node);
    let channel_issue = audio_issues
        .iter()
        .find(|issue| issue.code == LintCode::InvalidAudioChannel)
        .expect("audio behavior reports alias channel");
    let channel_fixes = node_quick_fixes_for_issue(&QuickFixCtx::new(&graph), channel_issue);
    assert!(channel_fixes
        .iter()
        .any(|fix| fix.fix_id == "audio_normalize_channel"));

    let missing_asset_issue = audio_issues
        .iter()
        .find(|issue| issue.code == LintCode::AudioAssetMissing)
        .expect("audio behavior reports missing asset");
    let missing_asset_fixes =
        node_quick_fixes_for_issue(&QuickFixCtx::new(&graph), missing_asset_issue);
    assert!(missing_asset_fixes
        .iter()
        .any(|fix| fix.fix_id == "audio_missing_asset_to_stop"));
}

#[test]
fn node_specs_keep_generic_payloads_on_the_existing_contract_path() {
    let ext_node = StoryNode::Generic(EventRaw::ExtCall {
        command: "hook".to_string(),
        args: vec![],
    });
    assert_eq!(node_kind_for_authoring_node(&ext_node), NodeKind::ExtCall);
    assert_eq!(
        node_spec_for_authoring_node(&ext_node)
            .capabilities
            .fidelity,
        FidelityClass::HostRequired
    );

    let generic_jump = StoryNode::Generic(EventRaw::Jump {
        target: "start".to_string(),
    });
    let generic_spec = node_spec_for_authoring_node(&generic_jump);
    assert_eq!(generic_spec.kind, NodeKind::GenericEvent);
    assert_eq!(
        generic_spec.capabilities.fidelity,
        FidelityClass::FallbackDegraded
    );
    assert!(!generic_spec.capabilities.export_supported);
}

#[test]
fn story_node_helpers_project_node_behavior_specs() {
    let samples = vec![
        StoryNode::Start,
        StoryNode::End,
        StoryNode::Dialogue {
            speaker: "A".to_string(),
            text: "B".to_string(),
        },
        StoryNode::Generic(EventRaw::ExtCall {
            command: "hook".to_string(),
            args: vec![],
        }),
        StoryNode::Generic(EventRaw::Jump {
            target: "legacy".to_string(),
        }),
    ];

    for node in samples {
        let spec = node_spec_for_authoring_node(&node);
        assert_eq!(node.type_name(), spec.display_name);
        assert_eq!(node.can_connect_to(), spec.ports.accepts_incoming);
        assert_eq!(
            node.can_connect_from(),
            spec.ports.output != PortOutput::None
        );
        assert_eq!(node.export_supported(), spec.capabilities.export_supported);
    }
}

#[test]
fn node_behavior_converts_raw_events_back_to_authoring_nodes() {
    let scene = EventRaw::Scene(SceneUpdateRaw {
        background: Some("bg/room.png".to_string()),
        music: Some("music/theme.ogg".to_string()),
        characters: vec![CharacterPlacementRaw {
            name: "Ava".to_string(),
            expression: Some("characters/ava.png".to_string()),
            ..Default::default()
        }],
    });

    assert_eq!(node_kind_for_event_raw(&scene), NodeKind::Scene);
    assert!(matches!(
        node_from_event_raw(&scene),
        StoryNode::Scene {
            profile: None,
            background: Some(_),
            music: Some(_),
            characters
        } if characters.len() == 1
    ));
    assert!(matches!(
        node_behavior_for_authoring_node(&node_from_event_raw(&scene)).event_to_node(&scene),
        Some(StoryNode::Scene { .. })
    ));

    let ext_call = EventRaw::ExtCall {
        command: "plugin.fade".to_string(),
        args: vec!["fast".to_string()],
    };
    assert_eq!(node_kind_for_event_raw(&ext_call), NodeKind::ExtCall);
    assert!(matches!(
        node_from_event_raw(&ext_call),
        StoryNode::Generic(EventRaw::ExtCall { .. })
    ));
    assert!(node_behavior_for_authoring_node(&StoryNode::Dialogue {
        speaker: "A".to_string(),
        text: "B".to_string(),
    })
    .event_to_node(&ext_call)
    .is_none());
}

#[test]
fn node_behavior_converts_authoring_nodes_to_raw_events_without_export_context() {
    let scene = StoryNode::Scene {
        profile: Some("draft_profile".to_string()),
        background: Some("bg/room.png".to_string()),
        music: Some("music/theme.ogg".to_string()),
        characters: vec![CharacterPlacementRaw {
            name: "Ava".to_string(),
            expression: Some("characters/ava.png".to_string()),
            ..Default::default()
        }],
    };

    let event = node_behavior_for_authoring_node(&scene)
        .to_event(&scene)
        .expect("scene converts without export context");
    assert!(matches!(
        event,
        EventRaw::Scene(SceneUpdateRaw {
            background: Some(_),
            music: Some(_),
            characters
        }) if characters.len() == 1
    ));
    assert!(matches!(
        node_to_event_raw_without_export_context(&scene),
        Ok(EventRaw::Scene(_))
    ));

    let choice = StoryNode::Choice {
        prompt: "Pick?".to_string(),
        options: vec!["A".to_string()],
    };
    assert_eq!(
        node_behavior_for_authoring_node(&choice).to_event(&choice),
        Err(NodeToEventError::RequiresExportContext)
    );

    assert_eq!(
        node_behavior_for_authoring_node(&StoryNode::Start).to_event(&StoryNode::Start),
        Err(NodeToEventError::PreviewOnlyMarker)
    );
}

#[test]
fn behavior_asset_refs_are_canonical_for_events_and_nodes() {
    let scene = EventRaw::Scene(SceneUpdateRaw {
        background: Some(" bg/room.png ".to_string()),
        music: Some("music/theme.ogg".to_string()),
        characters: vec![
            CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("characters/ava.png".to_string()),
                ..Default::default()
            },
            CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("characters/ava.png".to_string()),
                ..Default::default()
            },
        ],
    });
    let expected = vec![
        "bg/room.png".to_string(),
        "characters/ava.png".to_string(),
        "music/theme.ogg".to_string(),
    ];

    assert_eq!(event_asset_refs_for_raw(&scene), expected);
    assert_eq!(event_behavior_for_raw(&scene).asset_refs(&scene), expected);

    let audio = EventRaw::AudioAction(AudioActionRaw {
        channel: "sfx".to_string(),
        action: "play".to_string(),
        asset: Some(" sfx/click.ogg ".to_string()),
        volume: None,
        fade_duration_ms: None,
        loop_playback: None,
    });
    assert_eq!(
        event_asset_refs_for_raw(&audio),
        vec!["sfx/click.ogg".to_string()]
    );

    let node = StoryNode::Generic(audio);
    assert_eq!(
        node_asset_refs_for_authoring_node(&node),
        vec!["sfx/click.ogg".to_string()]
    );
    assert_eq!(
        node_behavior_for_authoring_node(&node).asset_refs(&node),
        vec!["sfx/click.ogg".to_string()]
    );
}
