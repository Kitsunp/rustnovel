use crate::editor::{AssetFieldTarget, StoryNode};

#[test]
fn low_z_order_image_is_background_layer() {
    let image = visual_novel_engine::EntityKind::Image(visual_novel_engine::ImageData {
        path: visual_novel_engine::runtime::SharedStr::from("bg/room.png"),
        tint: None,
    });
    assert!(crate::editor::scene_stage::is_background_image(
        &image, -100
    ));
    assert!(!crate::editor::scene_stage::is_background_image(&image, 0));
}

#[test]
fn dragged_character_payload_preserves_name_and_image_path() {
    let payload = "asset://char/furina\nassets/characters/furina.png";
    let parsed = super::DraggedAsset::parse(payload).expect("payload should parse");
    assert_eq!(parsed.kind, "char");
    assert_eq!(parsed.name, "furina");
    assert_eq!(parsed.path, "assets/characters/furina.png");
}

#[test]
fn dropped_background_assigns_to_selected_scene_instead_of_creating_duplicate_scene() {
    let scene = StoryNode::Scene {
        profile: None,
        background: None,
        music: None,
        characters: Vec::new(),
    };

    let assignment = super::assignment_for_dropped_asset(
        "bg",
        "assets/backgrounds/room.png",
        Some(9),
        Some(&scene),
    )
    .expect("selected scene should accept background drop");

    assert_eq!(
        assignment,
        (
            9,
            AssetFieldTarget::SceneBackground,
            "assets/backgrounds/room.png".to_string()
        )
    );
}

#[test]
fn dropped_audio_can_target_scene_music_patch_music_or_audio_node() {
    let scene = StoryNode::Scene {
        profile: None,
        background: None,
        music: None,
        characters: Vec::new(),
    };
    let patch = StoryNode::ScenePatch(Default::default());
    let audio = StoryNode::AudioAction {
        channel: "bgm".to_string(),
        action: "play".to_string(),
        asset: None,
        volume: None,
        fade_duration_ms: None,
        loop_playback: Some(true),
    };

    assert_eq!(
        super::assignment_for_dropped_asset(
            "audio",
            "assets/audio/theme.ogg",
            Some(1),
            Some(&scene)
        )
        .map(|(_, target, _)| target),
        Some(AssetFieldTarget::SceneMusic)
    );
    assert_eq!(
        super::assignment_for_dropped_asset(
            "audio",
            "assets/audio/theme.ogg",
            Some(2),
            Some(&patch)
        )
        .map(|(_, target, _)| target),
        Some(AssetFieldTarget::ScenePatchMusic)
    );
    assert_eq!(
        super::assignment_for_dropped_asset(
            "audio",
            "assets/audio/theme.ogg",
            Some(3),
            Some(&audio)
        )
        .map(|(_, target, _)| target),
        Some(AssetFieldTarget::AudioActionAsset)
    );
}

#[test]
fn dropped_character_targets_selected_scene_instead_of_creating_duplicate_patch() {
    let scene = StoryNode::Scene {
        profile: None,
        background: None,
        music: None,
        characters: Vec::new(),
    };

    assert!(super::assignment_for_dropped_asset(
        "char",
        "assets/characters/ava.png",
        Some(1),
        Some(&scene)
    )
    .is_none());
    assert_eq!(
        super::character_drop_target_node("char", Some(1), Some(&scene)),
        Some(1)
    );
}

#[test]
fn preview_source_label_marks_inherited_background_owner() {
    let (scene, owners) = inherited_background_scene();
    let selected = StoryNode::Scene {
        profile: None,
        background: None,
        music: None,
        characters: Vec::new(),
    };

    let label = super::preview_source_label(
        &scene,
        &None,
        crate::editor::ComposerPreviewMode::RuntimeInherited,
        Some(2),
        Some(&selected),
        &owners,
    );

    assert_eq!(label, "Preview: Runtime inherited");
}

#[test]
fn isolated_preview_mode_label_overrides_runtime_inheritance_badge() {
    let (scene, owners) = inherited_background_scene();
    let selected = StoryNode::Scene {
        profile: None,
        background: None,
        music: None,
        characters: Vec::new(),
    };

    let label = super::preview_source_label(
        &scene,
        &None,
        crate::editor::ComposerPreviewMode::IsolatedNode,
        Some(2),
        Some(&selected),
        &owners,
    );

    assert_eq!(label, "Preview: Isolated node");
}

#[test]
fn composer_chrome_layout_switches_to_multirow_controls_before_overlap() {
    let narrow = super::composer_chrome_layout(360.0);
    assert_eq!(narrow.mode, super::ComposerChromeMode::Narrow);
    assert!(!narrow.show_full_labels);
    assert!(narrow.preview_width <= 96.0);

    let compact = super::composer_chrome_layout(560.0);
    assert_eq!(compact.mode, super::ComposerChromeMode::Compact);
    assert!(!compact.show_full_labels);
    assert!(compact.preview_width < super::composer_chrome_layout(900.0).preview_width);

    let full = super::composer_chrome_layout(900.0);
    assert_eq!(full.mode, super::ComposerChromeMode::Full);
    assert!(full.show_full_labels);
}

#[test]
fn visual_composer_heading_shrinks_for_narrow_docks() {
    assert_eq!(super::visual_composer_heading_label(360.0), "Composer");
    assert_eq!(
        super::visual_composer_heading_label(560.0),
        "Visual Composer"
    );
}

#[test]
fn composer_stage_reserves_footer_without_collapsing_preview() {
    let unselected_stage = super::composer_stage_available_height(240.0, None);
    assert!(unselected_stage < 240.0);
    assert!(
        unselected_stage + super::composer_status_row_reserved_height(240.0) <= 240.0,
        "unselected composer frames should reserve the status row outside the stage"
    );

    let dialogue = StoryNode::Dialogue {
        speaker: "Sakura".to_string(),
        text: "Where should we go first?".to_string(),
    };
    let choice = StoryNode::Choice {
        prompt: "Where should Sakura take you first?".to_string(),
        options: vec![
            "Visit the courtyard".to_string(),
            "Find the music room".to_string(),
        ],
    };

    for selected_node in [&dialogue, &choice] {
        for visible_remaining in [180.0, 320.0, 540.0] {
            let stage_height =
                super::composer_stage_available_height(visible_remaining, Some(selected_node));
            let editor_height = super::composer_overlay_editor_available_height(
                visible_remaining,
                Some(selected_node),
            );
            let status_height = super::composer_status_row_reserved_height(visible_remaining);

            assert!(
                stage_height + status_height + editor_height <= visible_remaining + 0.001,
                "stage, status row and overlay editor must not overlap"
            );
            assert!(stage_height >= visible_remaining * 0.62);
            assert!(editor_height > 0.0);
            assert!(
                editor_height
                    <= crate::editor::visual_composer::overlay_editor::overlay_editor_reserved_height(
                        Some(selected_node),
                    ),
                "overlay editor should be bounded by the selected node content"
            );
        }
    }
}

#[test]
fn composer_viewport_uses_reserved_stage_height_without_double_status_deduction() {
    let viewport = super::viewport::composer_viewport_size(eframe::egui::vec2(560.0, 160.0), (1280.0, 720.0));

    assert!(
        viewport.y >= 159.0,
        "stage viewport should consume the reserved stage band instead of subtracting footer/status rows twice: {viewport:?}"
    );
}

#[test]
fn layer_panel_cedes_height_to_stage_and_overlay_editor_in_compact_composer() {
    let compact = super::composer_layer_list_height(260.0);
    let tall = super::composer_layer_list_height(720.0);

    assert!(compact > 0.0);
    assert!(compact < 60.0);
    assert!(tall > compact);
    assert!(tall <= 96.0);
}

fn inherited_background_scene() -> (
    visual_novel_engine::SceneState,
    std::collections::HashMap<u32, u32>,
) {
    let mut scene = visual_novel_engine::SceneState::new();
    let mut transform = visual_novel_engine::Transform::at(0, 0);
    transform.z_order = -100;
    let background_id = scene
        .spawn_with_transform(
            transform,
            visual_novel_engine::EntityKind::Image(visual_novel_engine::ImageData {
                path: visual_novel_engine::runtime::SharedStr::from("bg/previous.png"),
                tint: None,
            }),
        )
        .expect("background entity");
    let mut owners = std::collections::HashMap::new();
    owners.insert(background_id.raw(), 1);
    (scene, owners)
}
