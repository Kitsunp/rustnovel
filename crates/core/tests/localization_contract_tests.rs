use std::collections::BTreeMap;

use visual_novel_engine::{
    collect_script_localization_keys,
    runtime::{ChoiceOptionRaw, ChoiceRaw, DialogueRaw, EventRaw, ScriptRaw},
    LocalizationCatalog, LocalizationIssueKind,
};

#[test]
fn catalog_resolves_with_locale_fallback() {
    let mut catalog = LocalizationCatalog::new("en");
    catalog.insert_locale_table(
        "en",
        BTreeMap::from([
            ("dialogue.hello".to_string(), "Hello".to_string()),
            ("choice.yes".to_string(), "Yes".to_string()),
        ]),
    );
    catalog.insert_locale_table(
        "es",
        BTreeMap::from([("dialogue.hello".to_string(), "Hola".to_string())]),
    );

    assert_eq!(catalog.resolve("es", "dialogue.hello"), Some("Hola"));
    assert_eq!(catalog.resolve("es", "choice.yes"), Some("Yes"));
    assert_eq!(
        catalog.resolve_or_key("es", "choice.missing"),
        "choice.missing".to_string()
    );
}

#[test]
fn catalog_preserves_intentional_empty_translations() {
    let mut catalog = LocalizationCatalog::new("en");
    catalog.insert_locale_table(
        "en",
        BTreeMap::from([("ui.hidden_label".to_string(), String::new())]),
    );

    assert_eq!(catalog.resolve_or_key("en", "ui.hidden_label"), "");
    assert_eq!(catalog.resolve_or_key("en", "missing"), "missing");
}

#[test]
fn collect_script_keys_detects_loc_prefix() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::Dialogue(DialogueRaw {
                speaker: "loc:speaker.narrator".to_string(),
                text: "loc:dialogue.intro".to_string(),
            }),
            EventRaw::Choice(ChoiceRaw {
                prompt: "loc:choice.prompt".to_string(),
                options: vec![ChoiceOptionRaw {
                    text: "loc:choice.a".to_string(),
                    target: "start".to_string(),
                }],
            }),
        ],
        BTreeMap::from([("start".to_string(), 0usize)]),
    );

    let keys = collect_script_localization_keys(&script);
    assert!(keys.contains("speaker.narrator"));
    assert!(keys.contains("dialogue.intro"));
    assert!(keys.contains("choice.prompt"));
    assert!(keys.contains("choice.a"));
}

#[test]
fn validate_keys_reports_missing_and_orphan() {
    let mut catalog = LocalizationCatalog::new("en");
    catalog.insert_locale_table(
        "en",
        BTreeMap::from([
            ("dialogue.hello".to_string(), "Hello".to_string()),
            ("unused".to_string(), "unused".to_string()),
        ]),
    );

    let issues = catalog.validate_keys(["dialogue.hello", "dialogue.bye"]);
    assert!(issues.iter().any(|issue| {
        issue.locale == "en"
            && issue.key == "dialogue.bye"
            && issue.kind == LocalizationIssueKind::MissingKey
    }));
    assert!(issues.iter().any(|issue| {
        issue.locale == "en"
            && issue.key == "unused"
            && issue.kind == LocalizationIssueKind::OrphanKey
    }));
}

#[test]
fn validate_keys_reports_default_locale_missing_when_catalog_empty() {
    let catalog = LocalizationCatalog::new("en");

    let issues = catalog.validate_keys(["dialogue.intro"]);

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].locale, "en");
    assert_eq!(issues[0].key, "dialogue.intro");
    assert_eq!(issues[0].kind, LocalizationIssueKind::MissingKey);
}
