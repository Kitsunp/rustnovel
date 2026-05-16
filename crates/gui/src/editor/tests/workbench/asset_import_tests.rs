use super::super::*;
use crate::editor::{AssetFieldTarget, AssetImportKind, StoryNode};
use tempfile::tempdir;
use visual_novel_engine::{
    authoring::LintCode, CharacterPlacementRaw, ProjectManifest, ScenePatchRaw,
};

fn workbench_with_project(root: &std::path::Path) -> EditorWorkbench {
    let manifest = ProjectManifest::new("Import Test", "QA");
    let manifest_path = root.join("project.vnm");
    manifest.save(&manifest_path).expect("write manifest");

    let mut workbench = EditorWorkbench::new(VnConfig::default());
    workbench.project_root = Some(root.to_path_buf());
    workbench.manifest_path = Some(manifest_path);
    workbench.manifest = Some(manifest);
    workbench
}

#[test]
fn import_external_audio_copies_into_project_manifest() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    let outside_root = temp.path().join("outside");
    std::fs::create_dir_all(&project_root).expect("mkdir project");
    std::fs::create_dir_all(&outside_root).expect("mkdir outside");
    let source = outside_root.join("theme track.ogg");
    std::fs::write(&source, b"fake-audio").expect("write source");

    let mut workbench = workbench_with_project(&project_root);
    let imported = workbench
        .import_asset_file(&source, AssetImportKind::Audio)
        .expect("audio import");

    assert_eq!(imported, "assets/audio/theme_track.ogg");
    assert!(project_root.join(&imported).is_file());
    assert_eq!(
        workbench
            .manifest
            .as_ref()
            .and_then(|manifest| manifest.assets.audio.get("theme_track"))
            .map(|path| path.to_string_lossy().replace('\\', "/")),
        Some(imported)
    );
    let entry = workbench
        .operation_log
        .last()
        .expect("asset import should be logged");
    assert_eq!(entry.operation_kind, "asset_imported");
    assert!(matches!(
        entry.operation_kind_v2,
        Some(visual_novel_engine::authoring::OperationKind::AssetImported)
    ));
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some("manifest.assets.audio[theme_track]")
    );
    assert!(entry
        .before_value
        .as_deref()
        .is_some_and(|value| value.ends_with("theme track.ogg")));
    assert_eq!(
        entry.after_value.as_deref(),
        Some("assets/audio/theme_track.ogg")
    );
}

#[test]
fn imported_asset_can_be_assigned_to_selected_node_field() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    let outside_root = temp.path().join("outside");
    std::fs::create_dir_all(&project_root).expect("mkdir project");
    std::fs::create_dir_all(&outside_root).expect("mkdir outside");
    let source = outside_root.join("classroom.png");
    std::fs::write(&source, b"fake-image").expect("write source");

    let mut workbench = workbench_with_project(&project_root);
    let node_id = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 0.0),
    );
    workbench.refresh_operation_fingerprint();

    let imported = workbench
        .import_asset_file(&source, AssetImportKind::Background)
        .expect("background import");
    let before_assign = workbench.node_graph.clone();
    workbench
        .apply_imported_asset_to_node(node_id, AssetFieldTarget::SceneBackground, imported.clone())
        .expect("assign import");
    workbench.commit_modified_graph(before_assign);

    let Some(StoryNode::Scene { background, .. }) = workbench.node_graph.get_node(node_id) else {
        panic!("expected scene node");
    };
    assert_eq!(background.as_deref(), Some(imported.as_str()));
    assert!(!workbench.node_graph.is_modified());
    let entry = workbench
        .operation_log
        .last()
        .expect("asset assignment should be logged");
    assert_eq!(entry.operation_kind, "field_edited");
    assert!(matches!(
        entry.operation_kind_v2,
        Some(visual_novel_engine::authoring::OperationKind::FieldEdited)
    ));
    let expected_field_path = format!("graph.nodes[{node_id}].background");
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some(expected_field_path.as_str())
    );
    assert_eq!(entry.before_value.as_deref(), Some("<none>"));
    assert_eq!(entry.after_value.as_deref(), Some(imported.as_str()));
    assert!(entry.before_fingerprint_sha256.is_some());
    assert!(entry.after_fingerprint_sha256.is_some());
}

#[test]
fn import_rejects_unsupported_extension() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    let outside_root = temp.path().join("outside");
    std::fs::create_dir_all(&project_root).expect("mkdir project");
    std::fs::create_dir_all(&outside_root).expect("mkdir outside");
    let source = outside_root.join("script.exe");
    std::fs::write(&source, b"not-an-asset").expect("write source");

    let mut workbench = workbench_with_project(&project_root);
    let err = workbench
        .import_asset_file(&source, AssetImportKind::Audio)
        .expect_err("exe must be rejected");

    assert!(err.contains("unsupported .exe"));
}

#[test]
fn importing_asset_invalidates_cached_validation_for_existing_scene_reference() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    let outside_root = temp.path().join("outside");
    std::fs::create_dir_all(&project_root).expect("mkdir project");
    std::fs::create_dir_all(&outside_root).expect("mkdir outside");
    let source = outside_root.join("room.png");
    std::fs::write(&source, b"fake-image").expect("write source");

    let mut workbench = workbench_with_project(&project_root);
    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("assets/backgrounds/room.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 120.0),
    );
    let end = workbench
        .node_graph
        .add_node(StoryNode::End, egui::pos2(0.0, 240.0));
    workbench.node_graph.connect(start, scene);
    workbench.node_graph.connect(scene, end);

    assert!(!workbench.run_dry_validation());
    assert_eq!(workbench.compilation_cache_stats(), (0, 1));
    assert!(workbench
        .validation_issues
        .iter()
        .any(|issue| issue.code == LintCode::AssetReferenceMissing));

    let imported = workbench
        .import_asset_file(&source, AssetImportKind::Background)
        .expect("background import");
    assert_eq!(imported, "assets/backgrounds/room.png");

    assert!(workbench.run_dry_validation());
    assert_eq!(workbench.compilation_cache_stats(), (0, 2));
    assert!(workbench
        .validation_issues
        .iter()
        .all(|issue| issue.code != LintCode::AssetReferenceMissing));
}

#[test]
fn externally_created_referenced_asset_invalidates_cached_validation() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    std::fs::create_dir_all(&project_root).expect("mkdir project");

    let mut workbench = workbench_with_project(&project_root);
    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("assets/backgrounds/room.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 120.0),
    );
    let end = workbench
        .node_graph
        .add_node(StoryNode::End, egui::pos2(0.0, 240.0));
    workbench.node_graph.connect(start, scene);
    workbench.node_graph.connect(scene, end);

    assert!(!workbench.run_dry_validation());
    assert_eq!(workbench.compilation_cache_stats(), (0, 1));
    assert!(workbench
        .validation_issues
        .iter()
        .any(|issue| issue.code == LintCode::AssetReferenceMissing));

    let asset_path = project_root
        .join("assets")
        .join("backgrounds")
        .join("room.png");
    std::fs::create_dir_all(asset_path.parent().expect("asset parent")).expect("mkdir asset dir");
    std::fs::write(&asset_path, b"fake-image").expect("write external asset");

    assert!(workbench.run_dry_validation());
    assert_eq!(workbench.compilation_cache_stats(), (0, 2));
    assert!(workbench
        .validation_issues
        .iter()
        .all(|issue| issue.code != LintCode::AssetReferenceMissing));

    assert!(workbench.run_dry_validation());
    assert_eq!(workbench.compilation_cache_stats(), (1, 2));
}

#[test]
fn remove_asset_from_manifest_updates_sections_and_records_typed_operation() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    std::fs::create_dir_all(&project_root).expect("mkdir project");

    let mut workbench = workbench_with_project(&project_root);
    {
        let manifest = workbench.manifest.as_mut().expect("manifest");
        manifest.assets.backgrounds.insert(
            "room".to_string(),
            std::path::PathBuf::from("assets/backgrounds/room.png"),
        );
        manifest.assets.characters.insert(
            "ava".to_string(),
            visual_novel_engine::manifest::CharacterAsset {
                path: std::path::PathBuf::from("assets/characters/ava.png"),
                scale: Some(1.0),
            },
        );
        manifest.assets.audio.insert(
            "theme".to_string(),
            std::path::PathBuf::from("assets/audio/theme.ogg"),
        );
        manifest
            .save(workbench.manifest_path.as_ref().expect("manifest path"))
            .expect("save manifest");
    }
    workbench
        .composer_image_failures
        .insert("stale".to_string(), "missing".to_string());
    workbench.player_audio_root = Some(project_root.clone());

    workbench
        .remove_asset_from_manifest(AssetImportKind::Background, "room")
        .expect("remove background");
    workbench
        .remove_asset_from_manifest(AssetImportKind::Character, "ava")
        .expect("remove character");
    workbench
        .remove_asset_from_manifest(AssetImportKind::Audio, "theme")
        .expect("remove audio");

    let manifest = workbench.manifest.as_ref().expect("manifest");
    assert!(manifest.assets.backgrounds.is_empty());
    assert!(manifest.assets.characters.is_empty());
    assert!(manifest.assets.audio.is_empty());
    assert!(workbench.composer_image_failures.is_empty());
    assert!(workbench.player_audio_root.is_none());

    let saved = std::fs::read_to_string(workbench.manifest_path.as_ref().expect("manifest path"))
        .expect("read manifest");
    assert!(!saved.contains("room.png"));
    assert!(!saved.contains("ava.png"));
    assert!(!saved.contains("theme.ogg"));

    let entry = workbench
        .operation_log
        .last()
        .expect("asset removal should be logged");
    assert_eq!(entry.operation_kind, "asset_removed");
    assert!(matches!(
        entry.operation_kind_v2,
        Some(visual_novel_engine::authoring::OperationKind::AssetRemoved)
    ));
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some("manifest.assets.audio[theme]")
    );
    assert_eq!(
        entry.before_value.as_deref(),
        Some("assets/audio/theme.ogg")
    );
    assert_eq!(entry.after_value.as_deref(), Some("<removed>"));

    let err = workbench
        .remove_asset_from_manifest(AssetImportKind::Audio, "theme")
        .expect_err("missing asset removal should fail");
    assert!(err.contains("is not in the manifest"));
}

#[test]
fn imported_image_can_be_assigned_to_scene_character_expression() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    std::fs::create_dir_all(&project_root).expect("mkdir project");

    let mut workbench = workbench_with_project(&project_root);
    let node_id = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: vec![CharacterPlacementRaw {
                name: "Mira".to_string(),
                expression: None,
                position: None,
                x: None,
                y: None,
                scale: None,
            }],
        },
        egui::pos2(0.0, 0.0),
    );

    workbench
        .apply_imported_asset_to_node(
            node_id,
            AssetFieldTarget::SceneCharacterExpression(0),
            "assets/characters/mira.png".to_string(),
        )
        .expect("assign character expression");

    let Some(StoryNode::Scene { characters, .. }) = workbench.node_graph.get_node(node_id) else {
        panic!("expected scene node");
    };
    assert_eq!(
        characters[0].expression.as_deref(),
        Some("assets/characters/mira.png")
    );
}

#[test]
fn imported_image_can_be_assigned_to_scene_patch_background() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    std::fs::create_dir_all(&project_root).expect("mkdir project");

    let mut workbench = workbench_with_project(&project_root);
    let node_id = workbench.node_graph.add_node(
        StoryNode::ScenePatch(ScenePatchRaw {
            background: None,
            music: None,
            add: Vec::new(),
            update: Vec::new(),
            remove: Vec::new(),
        }),
        egui::pos2(0.0, 0.0),
    );

    workbench
        .apply_imported_asset_to_node(
            node_id,
            AssetFieldTarget::ScenePatchBackground,
            "assets/backgrounds/room.png".to_string(),
        )
        .expect("assign patch background");

    let Some(StoryNode::ScenePatch(patch)) = workbench.node_graph.get_node(node_id) else {
        panic!("expected scene patch node");
    };
    assert_eq!(
        patch.background.as_deref(),
        Some("assets/backgrounds/room.png")
    );
}

#[test]
fn dropped_character_adds_to_selected_scene_without_creating_extra_node_and_upserts_exact_asset() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    std::fs::create_dir_all(&project_root).expect("mkdir project");

    let mut workbench = workbench_with_project(&project_root);
    let node_id = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 0.0),
    );
    let before_nodes = workbench.node_graph.len();
    let before_assign = workbench.node_graph.clone();

    workbench
        .add_character_asset_to_node(
            node_id,
            "Ava".to_string(),
            "assets/characters/ava_happy.png".to_string(),
            320,
            240,
        )
        .expect("add character");
    workbench
        .add_character_asset_to_node(
            node_id,
            "Ava".to_string(),
            "assets/characters/ava_happy.png".to_string(),
            420,
            260,
        )
        .expect("upsert same character asset");
    workbench.commit_modified_graph(before_assign);

    assert_eq!(
        workbench.node_graph.len(),
        before_nodes,
        "dropping a character onto a selected scene should mutate that scene, not create a patch node"
    );
    let Some(StoryNode::Scene { characters, .. }) = workbench.node_graph.get_node(node_id) else {
        panic!("expected scene node");
    };
    assert_eq!(characters.len(), 1);
    assert_eq!(characters[0].name, "Ava");
    assert_eq!(
        characters[0].expression.as_deref(),
        Some("assets/characters/ava_happy.png")
    );
    assert_eq!(characters[0].x, Some(420));
    assert_eq!(characters[0].y, Some(260));

    let entry = workbench
        .operation_log
        .last()
        .expect("character assignment should be logged");
    assert_eq!(entry.operation_kind, "field_edited");
    let expected_field_path = format!("graph.nodes[{node_id}].characters[0]");
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some(expected_field_path.as_str())
    );
    assert!(entry
        .before_value
        .as_deref()
        .is_some_and(|value| value.contains("x=320")));
    assert!(entry
        .after_value
        .as_deref()
        .is_some_and(|value| value.contains("x=420")));
}

#[test]
fn dropped_same_name_different_character_assets_remain_distinct_scene_instances() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    std::fs::create_dir_all(&project_root).expect("mkdir project");

    let mut workbench = workbench_with_project(&project_root);
    let node_id = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 0.0),
    );

    workbench
        .add_character_asset_to_node(
            node_id,
            "Ava".to_string(),
            "assets/characters/ava_happy.png".to_string(),
            240,
            240,
        )
        .expect("add first pose");
    workbench
        .add_character_asset_to_node(
            node_id,
            "Ava".to_string(),
            "assets/characters/ava_angry.png".to_string(),
            520,
            240,
        )
        .expect("add second pose");

    let Some(StoryNode::Scene { characters, .. }) = workbench.node_graph.get_node(node_id) else {
        panic!("expected scene node");
    };
    assert_eq!(characters.len(), 2);
    assert_eq!(characters[0].name, characters[1].name);
    assert_ne!(characters[0].expression, characters[1].expression);
    assert_eq!(characters[0].x, Some(240));
    assert_eq!(characters[1].x, Some(520));
}
