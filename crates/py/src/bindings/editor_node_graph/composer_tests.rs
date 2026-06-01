use super::*;
use visual_novel_engine::authoring::{AuthoringPosition, StoryNode};

fn add_test_node(graph: &mut PyNodeGraph, node: StoryNode) -> u32 {
    let node_id = graph.inner.next_node_id();
    graph
        .apply_authoring_command(AuthoringCommand::CreateNode {
            node_id,
            node,
            position: AuthoringPosition::new(0.0, 0.0),
        })
        .expect("test node should be created");
    node_id
}

#[test]
fn set_choice_option_target_reports_invalid_node_and_option() {
    pyo3::prepare_freethreaded_python();

    let mut graph = PyNodeGraph::new();
    let dialogue = add_test_node(
        &mut graph,
        StoryNode::Dialogue {
            speaker: "A".to_string(),
            text: "Line".to_string(),
        },
    );

    let operation_count = graph.py_operation_log().len();
    let err = graph
        .py_set_choice_option_target(dialogue, 0, None)
        .expect_err("non-choice nodes must be reported as binding errors");
    assert!(err.to_string().contains("not a choice"));
    assert_eq!(graph.py_operation_log().len(), operation_count);

    let choice = add_test_node(
        &mut graph,
        StoryNode::Choice {
            prompt: "Pick".to_string(),
            options: vec!["Only".to_string()],
        },
    );

    let operation_count = graph.py_operation_log().len();
    let err = graph
        .py_set_choice_option_target(choice, 1, None)
        .expect_err("out-of-range choice options must be reported as binding errors");
    assert!(err.to_string().contains("no option 1"));
    assert_eq!(graph.py_operation_log().len(), operation_count);
}
