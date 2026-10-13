use visual_novel_engine::authoring::{
    composer::{self, ComposerPreviewSession},
    load_authoring_entry, parse_authoring_entry, save_authoring_document, AuthoringCommand,
    AuthoringDocument, AuthoringDocumentCommand, AuthoringDocumentSession, AuthoringPosition,
    NodeGraph, OperationKind, StoryNode,
};
use visual_novel_engine::{runtime::Engine, ResourceLimiter, SecurityPolicy};

#[test]
fn compound_edits_and_noops_preserve_the_document_history() {
    let mut session = AuthoringDocumentSession::new(AuthoringDocument::new(NodeGraph::new()));
    session
        .apply(AuthoringDocumentCommand::Graph(
            AuthoringCommand::CreateNode {
                node_id: 0,
                node: StoryNode::Start,
                position: AuthoringPosition::default(),
            },
        ))
        .unwrap();
    session
        .edit_graph(OperationKind::FieldEdited, "Create branch", None, |graph| {
            graph.create_branch(0)
        })
        .unwrap();
    let branched = session.document().graph.clone();
    assert_eq!(branched.len(), 4);
    assert_eq!(session.undo_delta_count(), 2);
    session.apply(AuthoringDocumentCommand::UndoLast).unwrap();
    assert_eq!(session.document().graph.len(), 1);
    let commands = session.recorded_commands().len();
    let (_, changed) = session
        .edit_graph(OperationKind::FieldEdited, "No change", None, |_| ())
        .unwrap();
    assert!(!changed);
    assert_eq!(session.recorded_commands().len(), commands);
    assert_eq!(session.redo_delta_count(), 1);
    session.apply(AuthoringDocumentCommand::RedoLast).unwrap();
    assert!(session.document().graph.has_same_content(&branched));
    session
        .edit_graph(OperationKind::FieldEdited, "Bookmark", None, |graph| {
            graph.set_bookmark("intro", 0)
        })
        .unwrap();
    session.apply(AuthoringDocumentCommand::UndoLast).unwrap();
    assert_eq!(session.document().graph.bookmarked_node("intro"), None);
    session.apply(AuthoringDocumentCommand::UndoLast).unwrap();
    assert_eq!(session.document().graph.len(), 1);
}

#[test]
fn shared_entry_roundtrip_preserves_layers_bookmarks_and_evidence() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("room.png".into()),
            music: None,
            characters: vec![],
        },
        AuthoringPosition::default(),
    );
    graph.set_bookmark("scene", scene);
    let object_id = composer::list_layered_objects(&graph, Some(scene))[0]
        .object_id
        .clone();
    let mut session = AuthoringDocumentSession::new(AuthoringDocument::new(graph));
    session
        .apply(AuthoringDocumentCommand::SetLayerVisible {
            object_id,
            visible: false,
        })
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("document.json");
    save_authoring_document(&path, session.document()).unwrap();
    let loaded = load_authoring_entry(&path).unwrap();
    assert_eq!(
        serde_json::to_value(&loaded).unwrap(),
        serde_json::to_value(session.document()).unwrap()
    );
    assert!(parse_authoring_entry(r#"{"authoring_schema_version":999,"graph":{}}"#).is_err());
    assert!(save_authoring_document(temp.path(), session.document()).is_err());
    assert_eq!(
        serde_json::to_value(load_authoring_entry(&path).unwrap()).unwrap(),
        serde_json::to_value(&loaded).unwrap()
    );
}

#[test]
fn preview_session_and_engine_adapter_deliver_identical_audio_and_state() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, AuthoringPosition::default());
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("room.png".into()),
            music: Some("theme.ogg".into()),
            characters: vec![],
        },
        AuthoringPosition::new(0.0, 100.0),
    );
    let line = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".into(),
            text: "Hello".into(),
        },
        AuthoringPosition::new(0.0, 200.0),
    );
    let end = graph.add_node(StoryNode::End, AuthoringPosition::new(0.0, 300.0));
    graph.connect(start, scene);
    graph.connect(scene, line);
    graph.connect(line, end);
    let mut preview = ComposerPreviewSession::start_from_node(&graph, scene).unwrap();
    let mut engine = Engine::new(
        graph.to_script_strict().unwrap(),
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let initial_audio =
        composer::jump_preview_engine(&mut engine, &format!("node_{scene}")).unwrap();
    assert!(
        !initial_audio.is_empty(),
        "scene music must be delivered to the client"
    );
    assert_eq!(preview.take_audio_commands(), initial_audio);
    assert!(preview.take_audio_commands().is_empty());
    let audio = composer::advance_preview_engine(&mut engine, None)
        .unwrap()
        .unwrap();
    preview.advance().unwrap();
    assert_eq!(preview.take_audio_commands(), audio);
    assert_eq!(
        serde_json::to_value(preview.snapshot(&graph, None, None, None)).unwrap(),
        serde_json::to_value(composer::compose_scene_snapshot(
            &graph,
            None,
            None,
            Some(&engine),
            None,
            None
        ))
        .unwrap(),
    );
}
