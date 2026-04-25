use super::super::*;
use crate::editor::{AssetFieldTarget, AssetImportKind, StoryNode};
use tempfile::tempdir;
use visual_novel_engine::{CharacterPlacementRaw, ProjectManifest, ScenePatchRaw};

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

    let imported = workbench
        .import_asset_file(&source, AssetImportKind::Background)
        .expect("background import");
    workbench
        .apply_imported_asset_to_node(node_id, AssetFieldTarget::SceneBackground, imported.clone())
        .expect("assign import");

    let Some(StoryNode::Scene { background, .. }) = workbench.node_graph.get_node(node_id) else {
        panic!("expected scene node");
    };
    assert_eq!(background.as_deref(), Some(imported.as_str()));
    assert!(workbench.node_graph.is_modified());
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
