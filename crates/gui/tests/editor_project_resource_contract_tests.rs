use std::fs;
use std::time::Duration;

use eframe::egui;
use tempfile::tempdir;
use visual_novel_engine::authoring::{
    build_authoring_report_fingerprint, composer::LayerOverride, OperationKind, OperationLogEntry,
    OperationStatus, VerificationRun,
};
use visual_novel_gui::editor::authoring_adapter::to_authoring_graph;
use visual_novel_gui::editor::project_io::{
    load_project, load_script, save_authoring_document_with_metadata, save_script,
};
use visual_novel_gui::editor::resource_service::EditorResourceService;
use visual_novel_gui::editor::{BackgroundFit, EditorError, NodeGraph, StoryNode};

#[test]
fn load_project_rejects_legacy_manifest_schema_alias() {
    let dir = tempdir().expect("tempdir");
    let manifest_path = dir.path().join("project.vnm");
    let script_path = dir.path().join("main.json");

    fs::write(
        &manifest_path,
        r#"
schema_version = "0.9"

[metadata]
name = "Legacy Project"
author = "QA"
version = "0.1.0"

[settings]
resolution = [1280, 720]
default_language = "es"
supported_languages = ["es", "en"]
entry_point = "main.json"

[assets]
"#,
    )
    .expect("write manifest");
    fs::write(
        &script_path,
        r#"{"script_schema_version":"1.0","events":[],"labels":{}}"#,
    )
    .expect("write script");

    let err = match load_project(manifest_path) {
        Ok(_) => panic!("legacy manifest must be rejected"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("missing manifest_schema_version"));
}

#[test]
fn load_project_rejects_entry_point_escape_outside_root() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("project");
    fs::create_dir_all(&project_root).expect("mkdir project");
    let manifest_path = project_root.join("project.vnm");
    let outside_script = dir.path().join("outside.json");

    fs::write(
        &outside_script,
        r#"{"script_schema_version":"1.0","events":[],"labels":{}}"#,
    )
    .expect("write outside script");
    fs::write(
        &manifest_path,
        r#"
manifest_schema_version = "1.0"

[metadata]
name = "Escape Test"
author = "QA"
version = "0.1.0"

[settings]
resolution = [1280, 720]
default_language = "en"
supported_languages = ["en"]
entry_point = "../outside.json"

[assets]
"#,
    )
    .expect("write manifest");

    match load_project(manifest_path) {
        Ok(_) => panic!("escape must be rejected"),
        Err(EditorError::CompileError(message)) => {
            assert!(message.contains("escapes project root"))
        }
        Err(other) => panic!("unexpected error: {other}"),
    }
}

#[test]
fn authoring_save_load_preserves_disconnected_draft_nodes() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("draft.vnauthoring");
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let live = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Connected".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    let draft = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Draft".to_string(),
            text: "Disconnected but important".to_string(),
        },
        egui::pos2(240.0, 100.0),
    );
    graph.connect(start, live);

    save_script(&path, &graph).expect("save authoring document");
    let saved = fs::read_to_string(&path).expect("read saved document");
    assert!(saved.contains("authoring_schema_version"));

    let loaded = load_script(path).expect("load authoring document");
    assert_eq!(loaded.graph.len(), graph.len());
    assert!(matches!(
        loaded.graph.get_node(draft),
        Some(StoryNode::Dialogue { speaker, text })
            if speaker == "Draft" && text == "Disconnected but important"
    ));
}

#[test]
fn authoring_save_load_preserves_operation_log_and_verification_runs() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("tracked.vnauthoring");
    let mut graph = NodeGraph::new();
    graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let operation = OperationLogEntry::new_typed(
        OperationKind::NodeCreated,
        OperationStatus::Applied,
        "created start node",
    );
    let authoring = to_authoring_graph(&graph);
    let verification = VerificationRun::from_diagnostics(
        &operation.operation_id,
        "gui-save",
        &build_authoring_report_fingerprint(
            &authoring,
            &authoring.to_script_lossy_for_diagnostics(),
        ),
        &[],
        &[],
    );

    save_authoring_document_with_metadata(
        &path,
        &graph,
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
        std::slice::from_ref(&operation),
        std::slice::from_ref(&verification),
    )
    .expect("save with metadata");

    let loaded = load_script(path).expect("load with metadata");
    assert_eq!(loaded.operation_log.len(), 1);
    assert_eq!(loaded.verification_runs.len(), 1);
    assert_eq!(
        loaded.operation_log[0].operation_kind,
        operation.operation_kind
    );
}

#[test]
fn authoring_save_load_preserves_composer_overrides() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("layers.vnauthoring");
    let mut graph = NodeGraph::new();
    graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let mut layer_overrides = std::collections::HashMap::new();
    layer_overrides.insert(
        "node:1:character:graph.nodes[1].visual.characters[0]:0".to_string(),
        LayerOverride {
            visible: false,
            locked: true,
        },
    );
    let mut background_overrides = std::collections::HashMap::new();
    background_overrides.insert("1".to_string(), BackgroundFit::Contain);

    save_authoring_document_with_metadata(
        &path,
        &graph,
        &layer_overrides,
        &background_overrides,
        &[],
        &[],
    )
    .expect("save composer overrides");
    let loaded = load_script(path).expect("load composer overrides");

    assert_eq!(loaded.composer_layer_overrides, layer_overrides);
    assert_eq!(
        loaded.composer_background_fit_overrides,
        background_overrides
    );
}

#[test]
fn asset_cache_multiview_stress() {
    let tmp = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(tmp.path().join("assets/backgrounds")).expect("asset dir");
    fs::write(
        tmp.path().join("assets/backgrounds/room.bin"),
        b"image-bytes",
    )
    .expect("asset");
    let mut service = EditorResourceService::new();

    for view in ["browser", "composer", "player"] {
        let bytes = service
            .image_bytes_for_view(tmp.path(), "assets/backgrounds/room.bin", view)
            .expect("image bytes");
        assert_eq!(bytes, b"image-bytes");
    }

    assert_eq!(service.metrics().misses, 1);
    assert_eq!(service.metrics().hits, 2);
    assert_eq!(service.metrics().decode_ms, 1);
}

#[test]
fn decoded_image_cache_multiview_and_fingerprint_invalidation() {
    let tmp = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(tmp.path().join("assets")).expect("asset dir");
    let path = tmp.path().join("assets/bg.png");
    fs::write(&path, tiny_png([255, 0, 0, 255])).expect("old png");
    let mut service = EditorResourceService::new();

    let first = service
        .image_for_view(tmp.path(), "assets/bg.png", "browser")
        .expect("first image");
    let decode_ms_after_first = service.metrics().decode_ms;
    let cached = service
        .image_for_view(tmp.path(), "assets/bg.png", "composer")
        .expect("cached image");
    assert_eq!(cached.pixels, first.pixels);
    assert_eq!(service.metrics().decode_ms, decode_ms_after_first);

    fs::write(&path, tiny_png([0, 0, 255, 255])).expect("new png");
    let second = service
        .image_for_view(tmp.path(), "assets/bg.png", "player")
        .expect("second image");
    assert_ne!(first.pixels, second.pixels);
    assert!(service.metrics().evictions >= 1);
}

#[test]
fn asset_cache_fingerprint_and_audio_metadata_invalidation() {
    let tmp = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(tmp.path().join("assets/audio")).expect("audio dir");
    let bg = tmp.path().join("assets/bg.bin");
    fs::create_dir_all(tmp.path().join("assets")).expect("asset dir");
    fs::write(&bg, b"old").expect("old asset");
    let mut service = EditorResourceService::new();

    assert_eq!(
        service
            .load_bytes(tmp.path(), "assets/bg.bin")
            .expect("old"),
        b"old"
    );
    fs::write(&bg, b"new").expect("new asset");
    assert_eq!(
        service
            .load_bytes(tmp.path(), "assets/bg.bin")
            .expect("new"),
        b"new"
    );

    let audio = tmp.path().join("assets/audio/tone.wav");
    fs::write(&audio, tiny_wav(Duration::from_millis(120), 8_000)).expect("wav");
    let first = service
        .audio_metadata(tmp.path(), "assets/audio/tone.wav")
        .expect("first metadata");
    fs::write(&audio, tiny_wav(Duration::from_millis(260), 8_000)).expect("wav updated");
    let second = service
        .audio_metadata(tmp.path(), "assets/audio/tone.wav")
        .expect("second metadata");

    assert_ne!(first.fingerprint, second.fingerprint);
    assert!(second.duration > first.duration);
    assert!(service.metrics().evictions >= 2);
}

fn tiny_wav(duration: Duration, sample_rate: u32) -> Vec<u8> {
    let samples = (duration.as_secs_f32() * sample_rate as f32).round() as u32;
    let data_len = samples * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(bytes.len() + data_len as usize, 0);
    bytes
}

fn tiny_png(rgba: [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba));
    let mut cursor = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, image::ImageOutputFormat::Png)
        .expect("encode png");
    cursor.into_inner()
}
