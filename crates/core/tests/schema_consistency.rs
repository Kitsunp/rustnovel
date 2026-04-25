use std::env;
use std::fs;
use std::path::PathBuf;

use schemars::schema_for;
use visual_novel_engine::EventCompiled;
use visual_novel_engine::ScriptRaw;

fn verify_schema<T: schemars::JsonSchema>(name: &str) {
    let schema = schema_for!(T);
    let schema_json = serde_json::to_string_pretty(&schema).unwrap();

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(format!("schema_{}.json", name));

    if env::var("UPDATE_SCHEMA").is_ok() || !path.exists() {
        fs::write(&path, schema_json).expect("failed to write schema");
        return;
    }

    let existing_json = fs::read_to_string(&path).expect("failed to read existing schema");

    // Normalize newlines
    let schema_json = schema_json.replace("\r\n", "\n");
    let existing_json = existing_json.replace("\r\n", "\n");

    if schema_json != existing_json {
        panic!(
            "Schema mismatch for {}. Run with UPDATE_SCHEMA=1 to update.\nExpected:\n{}\nActual:\n{}",
            name, existing_json, schema_json
        );
    }
}

#[test]
fn test_event_compiled_schema_snapshot() {
    verify_schema::<EventCompiled>("event_compiled");
}

#[test]
fn test_script_raw_schema_snapshot() {
    verify_schema::<ScriptRaw>("script_raw");
}
