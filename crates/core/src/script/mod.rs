mod compiled;
mod raw;

pub use compiled::ScriptCompiled;
pub use raw::ScriptRaw;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::ResourceLimiter;

    #[test]
    fn test_huge_script_rejection() {
        // Engineer Manifesto: Resource Budgeting & Double Representation.
        // Ensure that extremely large scripts are rejected *before* full processing.

        let limits = ResourceLimiter {
            max_script_bytes: 100, // Very small limit
            ..Default::default()
        };

        // Construct a script that exceeds the budget
        let mut huge_text = String::new();
        for _ in 0..200 {
            huge_text.push('a');
        }

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

        // Should fail with ResourceLimit
        let result = ScriptRaw::from_json_with_limits(&json, limits);
        match result {
            Err(crate::error::VnError::ResourceLimit(_)) => {}
            _ => panic!("Should have failed with ResourceLimit, got {:?}", result),
        }
    }
}
