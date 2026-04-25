mod common;
use common::run_headless;

#[test]
fn golden_script_incremental_patch() {
    let script_json = r#"
    {
        "script_schema_version": "1.0",
        "events": [
            {
                "type": "scene",
                "background": "bg_city",
                "music": "music_day",
                "characters": [
                    {"name": "Alice", "expression": "smile", "position": "left"}
                ]
            },
            {"type": "dialogue", "speaker": "Alice", "text": "Hello!"},
            {
                "type": "patch",
                "add": [
                    {"name": "Bob", "expression": "neutral", "position": "right"}
                ],
                "update": [
                    {"name": "Alice", "expression": "surprised", "position": "left"}
                ],
                "remove": []
            },
            {"type": "dialogue", "speaker": "Bob", "text": "Hi Alice."},
            {
                "type": "patch",
                "background": "bg_night",
                "add": [],
                "update": [],
                "remove": []
            },
            {"type": "dialogue", "speaker": "Alice", "text": "It got dark."},
            {
                "type": "patch",
                "music": "music_night",
                "add": [],
                "update": [],
                "remove": ["Alice", "Bob"]
            },
            {"type": "dialogue", "speaker": "Narrator", "text": "They left."}
        ],
        "labels": {
            "start": 0
        }
    }
    "#;

    let trace = run_headless(script_json, 15);
    insta::assert_yaml_snapshot!(trace);
}
