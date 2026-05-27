use visual_novel_engine::{
    manifest::{
        migrate_manifest_toml_to_current, migrate_manifest_value_to_current,
        MANIFEST_SCHEMA_VERSION,
    },
    ProjectManifest,
};

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
fn manifest_migration_current_schema_is_idempotent() {
    let current = ProjectManifest::new("Proyecto", "Autor");
    let source = toml::to_string(&current).expect("current manifest should serialize");

    let (first, first_report) =
        migrate_manifest_toml_to_current(&source).expect("first migration should succeed");
    let (second, second_report) =
        migrate_manifest_toml_to_current(&first).expect("second migration should succeed");

    assert_eq!(first, second);
    assert!(!first_report.changed());
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
fn from_toml_with_migration_rejects_legacy_without_schema() {
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

    let err = ProjectManifest::from_toml_with_migration(legacy)
        .expect_err("legacy manifest without schema must be rejected");
    assert!(err.to_string().contains("missing manifest_schema_version"));
}

#[test]
fn from_toml_with_migration_rejects_legacy_schema_alias() {
    let legacy = r#"
schema_version = "1.0"

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

    let err = ProjectManifest::from_toml_with_migration(legacy)
        .expect_err("legacy schema_version alias must be rejected");
    assert!(err.to_string().contains("missing manifest_schema_version"));
}
