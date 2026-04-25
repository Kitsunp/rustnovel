use super::*;

#[test]
fn test_manifest_roundtrip() {
    let manifest = ProjectManifest::new("Test Project", "Tester");
    let toml_str = toml::to_string(&manifest).expect("Failed to serialize");
    let loaded: ProjectManifest = toml::from_str(&toml_str).expect("Failed to deserialize");

    assert_eq!(manifest, loaded);
    assert_eq!(loaded.metadata.name, "Test Project");
    assert_eq!(loaded.manifest_schema_version, MANIFEST_SCHEMA_VERSION);
}

#[test]
fn test_settings_defaults() {
    let manifest = ProjectManifest::new("P", "A");
    assert_eq!(manifest.settings.resolution, (1280, 720));
    assert_eq!(manifest.settings.default_language, "en");
    assert_eq!(manifest.settings.entry_point, "main.json");
}

#[test]
fn manifest_migration_idempotent() {
    let legacy = r#"
[metadata]
name = "Proyecto"
author = "Autor"
version = "0.1.0"

[settings]
resolution = [1280, 720]
default_language = "es"
supported_languages = ["es", "en"]
entry_point = "main.json"

[assets]
"#;

    let (first, first_report) =
        migrate_manifest_toml_to_current(legacy).expect("first migration should succeed");
    let (second, second_report) =
        migrate_manifest_toml_to_current(&first).expect("second migration should succeed");

    assert_eq!(first, second);
    assert!(first_report.changed());
    assert!(!second_report.changed());
}

#[test]
fn manifest_migration_rollback_on_failure() {
    let mut invalid = toml::Value::String("bad".to_string());
    let snapshot = invalid.clone();
    let err = migrate_manifest_value_to_current(&mut invalid).expect_err("migration should fail");
    assert_eq!(invalid, snapshot, "migration must rollback on failure");
    assert!(err.to_string().contains("TOML table"));
}

#[test]
fn from_toml_with_migration_accepts_legacy_without_schema() {
    let legacy = r#"
[metadata]
name = "Proyecto"
author = "Autor"
version = "0.1.0"

[settings]
resolution = [1280, 720]
default_language = "es"
supported_languages = ["es", "en"]
entry_point = "main.json"

[assets]
"#;

    let (manifest, report) =
        ProjectManifest::from_toml_with_migration(legacy).expect("legacy manifest should load");
    assert_eq!(manifest.manifest_schema_version, MANIFEST_SCHEMA_VERSION);
    assert!(report.changed());
}
