use visual_novel_engine::{LruCache, ResourceLimiter, ScriptRaw, VnError, SCRIPT_SCHEMA_VERSION};

#[test]
fn test_lru_eviction() {
    let mut cache = LruCache::new(10);
    cache.insert(1u32, vec![1; 4]);
    cache.insert(2u32, vec![2; 4]);
    let _ = cache.get(&1u32);
    cache.insert(3u32, vec![3; 4]);

    assert!(cache.get(&2u32).is_none());
    assert!(cache.get(&1u32).is_some());
    assert!(cache.get(&3u32).is_some());
    assert_eq!(cache.current_bytes(), 8);
}

#[test]
fn test_huge_script_rejection() {
    let limits = ResourceLimiter {
        max_script_bytes: 32,
        ..ResourceLimiter::default()
    };
    let huge_text = "a".repeat(64);
    let script_json = format!(
        r#"{{
  "script_schema_version": "{schema}",
  "events": [
    {{ "type": "dialogue", "speaker": "A", "text": "{text}" }}
  ],
  "labels": {{ "start": 0 }}
}}"#,
        schema = SCRIPT_SCHEMA_VERSION,
        text = huge_text
    );
    let result = ScriptRaw::from_json_with_limits(&script_json, limits);
    assert!(matches!(result, Err(VnError::ResourceLimit(_))));
}

#[test]
fn test_oversized_invalid_json_rejects_before_deserialize() {
    let limits = ResourceLimiter {
        max_script_bytes: 16,
        ..ResourceLimiter::default()
    };
    let oversized_invalid_json = "{".repeat(64);

    let result = ScriptRaw::from_json_with_limits(&oversized_invalid_json, limits);
    assert!(matches!(result, Err(VnError::ResourceLimit(_))));
}
