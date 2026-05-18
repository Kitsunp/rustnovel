use visual_novel_engine::authoring::{
    validate_authoring_graph_with_resolver, AuthoringPosition, LintCode, NodeGraph, StoryNode,
    TraceAtomKind,
};

#[test]
fn asset_missing_trace_keeps_raw_value_and_resolver_metadata() {
    let mut graph = NodeGraph::new();
    graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/missing.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        AuthoringPosition::new(0.0, 0.0),
    );

    let issue = validate_authoring_graph_with_resolver(&graph, |_asset| false)
        .into_iter()
        .find(|issue| issue.code == LintCode::AssetReferenceMissing)
        .expect("missing asset diagnostic");
    let trace = issue.evidence_trace.expect("evidence trace");

    let resolver = trace
        .atoms
        .iter()
        .find(|atom| atom.kind == TraceAtomKind::ResolverLookup)
        .expect("resolver atom");
    assert_eq!(
        resolver.metadata.get("resolver_kind").map(String::as_str),
        Some("asset_resolver")
    );
    assert_eq!(
        resolver.metadata.get("resolver_inputs").map(String::as_str),
        Some("bg/missing.png")
    );

    let value = trace
        .atoms
        .iter()
        .find(|atom| atom.kind == TraceAtomKind::ValueRead)
        .expect("value atom");
    assert_eq!(
        value.metadata.get("actual_raw").map(String::as_str),
        Some("bg/missing.png")
    );
    assert_eq!(
        value.metadata.get("actual_normalized").map(String::as_str),
        Some("bg/missing.png")
    );
}
