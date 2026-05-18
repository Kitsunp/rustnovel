use crate::editor::{AssetFieldTarget, StoryNode};

#[test]
fn low_z_order_image_is_background_layer() {
    let image = visual_novel_engine::EntityKind::Image(visual_novel_engine::ImageData {
        path: visual_novel_engine::SharedStr::from("bg/room.png"),
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
                path: visual_novel_engine::SharedStr::from("bg/previous.png"),
                tint: None,
            }),
        )
        .expect("background entity");
    let mut owners = std::collections::HashMap::new();
    owners.insert(background_id.raw(), 1);
    (scene, owners)
}
