use serde_json::json;
use visual_novel_engine::authoring::*;

#[test]
fn command_json_round_trips_graph_and_existing_document_commands() {
    let commands = [
        json!({"command":"graph", "operation":"create_node", "node_id":0,
            "node":"Start", "position":{"x":0.0,"y":0.0}}),
        json!({"command":"graph", "operation":"connect", "from":0,"from_port":0,"to":1}),
        json!({"command":"graph", "operation":"edit_dialogue", "node_id":1,
            "speaker":"Ava","text":"Hello"}),
        json!({"command":"graph", "operation":"move_layer", "object_id":"layer",
            "x":100,"y":200,"scale":1.5}),
        json!({"command":"set_layer_visible","object_id":"layer","visible":false}),
        json!({"command":"set_layer_locked","object_id":"layer","locked":true}),
        json!({"command":"set_background_fit_override","node_id":0,"fit":"contain"}),
        json!({"command":"clear_background_fit_override","node_id":0}),
        json!({"command":"reset_composer_overrides"}),
        json!({"command":"undo_last"}),
        json!({"command":"redo_last"}),
    ];
    for value in commands {
        let command: AuthoringDocumentCommand = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(command).unwrap(), value);
    }
}

#[test]
fn command_json_rejects_unknown_fields_and_internal_diagnostic_objects() {
    for value in [
        json!({"command":"set_layer_locked","object_id":"layer","locked":true,"typo":1}),
        json!({"command":"graph","operation":"edit_dialogue","node_id":0,
            "speaker":"Ava","text":"Hello","typo":1}),
        json!({"command":"graph","operation":"apply_quick_fix","issue":{},"fix_id":"fix"}),
    ] {
        assert!(serde_json::from_value::<AuthoringDocumentCommand>(value).is_err());
    }
}

#[test]
fn headless_batch_rejects_empty_input_and_reports_failing_command() {
    let document = AuthoringDocument::new(NodeGraph::new());
    assert!(
        apply_authoring_document_commands_headless(document.clone(), vec![])
            .unwrap_err()
            .contains("empty")
    );
    let error = apply_authoring_document_commands_headless(
        document,
        vec![
            AuthoringDocumentCommand::Graph(AuthoringCommand::CreateNode {
                node_id: 0,
                node: StoryNode::Start,
                position: AuthoringPosition::new(0.0, 0.0),
            }),
            AuthoringDocumentCommand::Graph(AuthoringCommand::RemoveNode { node_id: 999 }),
        ],
    )
    .unwrap_err();
    assert!(error.contains("command 2 failed"), "{error}");
}
