use visual_novel_engine::{runtime::ScriptRaw, ResourceLimiter, VnError};

#[test]
fn huge_script_rejection_happens_before_full_processing() {
    let limits = ResourceLimiter {
        max_script_bytes: 100,
        ..Default::default()
    };
    let huge_text = "a".repeat(200);
    let json = format!(
        r#"{{
            "script_schema_version": "1.0",
            "events": [
                {{
                    "type": "dialogue",
                    "speaker": "Me",
                    "text": "{}"
                }}
            ],
            "labels": {{ "start": 0 }}
        }}"#,
        huge_text
    );

    let result = ScriptRaw::from_json_with_limits(&json, limits);
    assert!(matches!(result, Err(VnError::ResourceLimit(_))));
}
