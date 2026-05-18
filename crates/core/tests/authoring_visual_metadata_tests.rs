use visual_novel_engine::authoring::{
    build_authoring_document_report_fingerprint, composer::BackgroundFit, AuthoringDocument,
    AuthoringPosition, NodeGraph, StoryNode,
};

#[test]
fn background_fit_override_changes_layout_only_not_story_semantics() {
    let mut graph = NodeGraph::new();
    graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("assets/backgrounds/room.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        AuthoringPosition::new(0.0, 0.0),
    );
    let script = graph.to_script_lossy_for_diagnostics();
    let before_doc = AuthoringDocument::new(graph.clone());
    let mut after_doc = AuthoringDocument::new(graph);
    after_doc
        .composer_background_fit_overrides
        .insert("1".to_string(), BackgroundFit::Contain);

    let before = build_authoring_document_report_fingerprint(&before_doc, &script);
    let after = build_authoring_document_report_fingerprint(&after_doc, &script);

    assert_eq!(before.story_semantic_sha256, after.story_semantic_sha256);
    assert_ne!(before.layout_sha256, after.layout_sha256);
    assert_ne!(before.full_document_sha256, after.full_document_sha256);
}
