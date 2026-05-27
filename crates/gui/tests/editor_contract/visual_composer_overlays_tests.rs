use super::*;

#[test]
fn choice_layout_grows_rows_for_long_options_without_panel_overflow() {
    let stage = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 360.0));
    let options = vec![
        "A short option".to_string(),
        "A much longer option that should wrap inside the Visual Composer choice overlay instead of escaping the button bounds".to_string(),
    ];

    let layout = choice_overlay_layout(stage, "A prompt that can also wrap safely", &options);

    assert!(layout.panel.width() <= stage.width());
    assert!(layout.panel.height() <= stage.height() - 48.0);
    assert!(layout.option_heights[1] > layout.option_heights[0]);
    assert!(layout.options_viewport_height <= layout.panel.height());
}

#[test]
fn choice_layout_uses_scroll_viewport_for_many_options() {
    let stage = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(480.0, 280.0));
    let options = (0..12)
        .map(|idx| format!("Route {idx} with enough text to need a stable wrapped row"))
        .collect::<Vec<_>>();

    let layout = choice_overlay_layout(stage, "Pick a route", &options);
    let total_rows = layout.option_heights.iter().sum::<f32>()
        + 8.0 * layout.option_heights.len().saturating_sub(1) as f32;

    assert!(layout.panel.width() <= stage.width());
    assert!(layout.panel.height() <= stage.height());
    assert!(layout.panel.min.x >= stage.min.x);
    assert!(layout.panel.max.x <= stage.max.x);
    assert!(layout.panel.min.y >= stage.min.y);
    assert!(layout.panel.max.y <= stage.max.y);
    assert!(total_rows > layout.options_viewport_height);
}

#[test]
fn choice_layout_handles_tiny_stage_and_unbroken_text_without_invalid_geometry() {
    let stage = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(120.0, 72.0));
    let options = (0..24)
        .map(|idx| format!("Route{idx}{}", "x".repeat(320)))
        .collect::<Vec<_>>();

    let layout = choice_overlay_layout(stage, &"P".repeat(480), &options);

    assert!(layout.panel.width().is_finite());
    assert!(layout.panel.height().is_finite());
    assert!(layout.prompt_height.is_finite());
    assert!(layout.options_viewport_height.is_finite());
    assert!(layout.panel.width() <= stage.width());
    assert!(layout.panel.height() <= stage.height());
    assert!(layout.panel.min.x >= stage.min.x);
    assert!(layout.panel.max.x <= stage.max.x);
    assert!(layout.panel.min.y >= stage.min.y);
    assert!(layout.panel.max.y <= stage.max.y);
    assert!(layout
        .option_heights
        .iter()
        .all(|height| *height > 0.0 && height.is_finite()));
}

#[test]
fn soft_wrap_long_tokens_inserts_invisible_breaks_without_changing_words_with_spaces() {
    let unbroken = "x".repeat(96);
    let wrapped = soft_wrap_long_tokens(&unbroken, 24);
    assert_eq!(wrapped.replace('\u{200b}', ""), unbroken);
    assert!(wrapped.contains('\u{200b}'));

    let spaced = "short words stay readable";
    assert_eq!(soft_wrap_long_tokens(spaced, 24), spaced);
}

#[test]
fn selected_authoring_choice_overlay_takes_precedence_over_stale_runtime_dialogue() {
    let script = visual_novel_engine::runtime::ScriptRaw::new(
        vec![visual_novel_engine::runtime::EventRaw::Dialogue(
            visual_novel_engine::runtime::DialogueRaw {
                speaker: "Runtime".to_string(),
                text: "This runtime event is not the selected graph node".to_string(),
            },
        )],
        std::collections::BTreeMap::from([("start".to_string(), 0usize)]),
    );
    let engine = visual_novel_engine::runtime::Engine::new(
        script,
        visual_novel_engine::SecurityPolicy::default(),
        visual_novel_engine::ResourceLimiter::default(),
    )
    .expect("engine");
    let selected = StoryNode::Choice {
        prompt: "Selected choice".to_string(),
        options: vec!["A".to_string(), "B".to_string()],
    };

    let source = selected_overlay_source(
        Some(&engine),
        Some(&selected),
        ComposerPreviewMode::IsolatedNode,
        &HashMap::new(),
    )
    .expect("selected choice overlay");

    assert_eq!(
        source,
        OverlaySource::Choice {
            prompt: "Selected choice".to_string(),
            options: vec!["A".to_string(), "B".to_string()]
        }
    );
}

#[test]
fn hidden_authoring_choice_overlay_falls_back_to_runtime_overlay() {
    let script = visual_novel_engine::runtime::ScriptRaw::new(
        vec![visual_novel_engine::runtime::EventRaw::Dialogue(
            visual_novel_engine::runtime::DialogueRaw {
                speaker: "Runtime".to_string(),
                text: "Visible runtime dialogue".to_string(),
            },
        )],
        std::collections::BTreeMap::from([("start".to_string(), 0usize)]),
    );
    let engine = visual_novel_engine::runtime::Engine::new(
        script,
        visual_novel_engine::SecurityPolicy::default(),
        visual_novel_engine::ResourceLimiter::default(),
    )
    .expect("engine");
    let selected = StoryNode::Choice {
        prompt: "Hidden selected choice".to_string(),
        options: vec!["A".to_string()],
    };
    let mut overrides = HashMap::new();
    overrides.insert(
        "overlay:choice".to_string(),
        LayerOverride {
            visible: false,
            locked: false,
        },
    );

    let source = selected_overlay_source(
        Some(&engine),
        Some(&selected),
        ComposerPreviewMode::IsolatedNode,
        &overrides,
    )
    .expect("runtime fallback overlay");

    assert_eq!(
        source,
        OverlaySource::Dialogue {
            speaker: "Runtime".to_string(),
            text: "Visible runtime dialogue".to_string()
        }
    );
}

#[test]
fn runtime_overlay_visibility_uses_matching_layer_override() {
    let event = EventCompiled::Choice(visual_novel_engine::runtime::ChoiceCompiled {
        prompt: visual_novel_engine::runtime::SharedStr::from("Pick one"),
        options: Vec::new(),
    });
    let mut overrides = HashMap::new();
    assert!(runtime_overlay_visible(&event, &overrides));

    overrides.insert(
        "overlay:choice".to_string(),
        LayerOverride {
            visible: false,
            locked: true,
        },
    );
    assert!(!runtime_overlay_visible(&event, &overrides));
}

#[test]
fn runtime_preview_mode_uses_engine_overlay_instead_of_selected_authoring_node() {
    let script = visual_novel_engine::runtime::ScriptRaw::new(
        vec![visual_novel_engine::runtime::EventRaw::Choice(
            visual_novel_engine::runtime::ChoiceRaw {
                prompt: "Runtime choice".to_string(),
                options: vec![visual_novel_engine::runtime::ChoiceOptionRaw {
                    text: "Runtime route".to_string(),
                    target: "__end".to_string(),
                }],
            },
        )],
        std::collections::BTreeMap::from([
            ("start".to_string(), 0usize),
            ("__end".to_string(), 1usize),
        ]),
    );
    let engine = visual_novel_engine::runtime::Engine::new(
        script,
        visual_novel_engine::SecurityPolicy::default(),
        visual_novel_engine::ResourceLimiter::default(),
    )
    .expect("engine");
    let selected = StoryNode::Dialogue {
        speaker: "Selected".to_string(),
        text: "This is not the runtime event".to_string(),
    };

    let source = selected_overlay_source(
        Some(&engine),
        Some(&selected),
        ComposerPreviewMode::RuntimeInherited,
        &HashMap::new(),
    )
    .expect("runtime choice overlay");

    assert_eq!(
        source,
        OverlaySource::Choice {
            prompt: "Runtime choice".to_string(),
            options: vec!["Runtime route".to_string()],
        }
    );
}
