use visual_novel_engine::{compute_script_id, ScriptRaw, SCRIPT_SCHEMA_VERSION};

#[test]
fn compiled_script_id_is_deterministic() {
    let script_json = format!(
        r#"{{
            "script_schema_version": "{SCRIPT_SCHEMA_VERSION}",
            "events": [
                {{"type": "dialogue", "speaker": "Narrator", "text": "Hi"}},
                {{"type": "set_flag", "key": "seen", "value": true}}
            ],
            "labels": {{"start": 0}}
        }}"#
    );
    let script = ScriptRaw::from_json(&script_json).expect("parse");
    let compiled = script.compile().expect("compile");
    let bytes_one = compiled.to_binary().expect("serialize");
    let id_one = compute_script_id(&bytes_one);

    let compiled_again = script.compile().expect("compile");
    let bytes_two = compiled_again.to_binary().expect("serialize");
    let id_two = compute_script_id(&bytes_two);

    assert_eq!(bytes_one, bytes_two);
    assert_eq!(id_one, id_two);
}
