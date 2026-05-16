use visual_novel_engine::authoring::composer::compose_scene_snapshot;
use visual_novel_engine::authoring::{
    validate_authoring_graph_no_io, AuthoringPosition, StoryNode,
};

use super::*;

#[test]
fn python_report_v2_wrapper_roundtrips_and_explains() {
    let mut graph = NodeGraph::new();
    graph.add_node(
        StoryNode::Choice {
            prompt: "Route?".to_string(),
            options: vec!["Option 1".to_string()],
        },
        AuthoringPosition::new(0.0, 0.0),
    );
    let issues = validate_authoring_graph_no_io(&graph);
    let report = AuthoringValidationReport::from_graph_and_issues(
        &graph,
        &graph.to_script_lossy_for_diagnostics(),
        &issues,
    );
    let py_report = PyAuthoringValidationReport::from(report.clone());
    let json = py_report.to_json().expect("json");
    let parsed = PyAuthoringValidationReport::from_json(&json).expect("parse");

    assert_eq!(parsed.schema(), "vnengine.authoring_validation_report.v2");
    assert_eq!(parsed.issue_count(), report.issue_count);
    assert_eq!(parsed.issues().len(), report.issues.len());
    assert!(parsed
        .issues_json()
        .expect("issues json")
        .contains(&report.issues[0].diagnostic_id));
    assert!(parsed
        .explain(&report.issues[0].diagnostic_id)
        .expect("explain")
        .contains("typed_message_args"));
    assert!(!parsed
        .is_stale_against(&py_report.fingerprints_json().expect("fingerprints"))
        .expect("stale compare"));

    let mut changed_graph = graph.clone();
    changed_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "New semantic node".to_string(),
        },
        AuthoringPosition::new(120.0, 0.0),
    );
    let changed_report =
        PyAuthoringValidationReport::from(AuthoringValidationReport::from_graph_and_issues(
            &changed_graph,
            &changed_graph.to_script_lossy_for_diagnostics(),
            &validate_authoring_graph_no_io(&changed_graph),
        ));
    assert!(parsed
        .is_stale_against_report(&changed_report)
        .expect("report stale compare"));
}

#[test]
fn python_fragment_and_composer_wrappers_expose_stable_json() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        AuthoringPosition::new(0.0, 0.0),
    );
    assert!(graph.create_fragment("scene", "Scene", vec![scene]));

    let fragment = PyGraphFragment::from(graph.list_fragments()[0].clone());
    assert_eq!(fragment.fragment_id(), "scene");
    assert!(fragment.to_json().expect("fragment json").contains("Scene"));

    let snapshot = PyComposerSnapshot::from(compose_scene_snapshot(
        &graph,
        Some(scene),
        None,
        None,
        None,
        None,
    ));
    assert_eq!(snapshot.objects().len(), 1);
    assert!(snapshot
        .to_json()
        .expect("snapshot json")
        .contains("bg/room.png"));
}
