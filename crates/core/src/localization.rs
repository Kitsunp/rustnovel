use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::event::EventRaw;
use crate::script::ScriptRaw;

const LOC_PREFIX: &str = "loc:";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizationCatalog {
    pub default_locale: String,
    pub locales: BTreeMap<String, BTreeMap<String, String>>,
}

impl Default for LocalizationCatalog {
    fn default() -> Self {
        Self {
            default_locale: "en".to_string(),
            locales: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalizationIssue {
    pub locale: String,
    pub key: String,
    pub kind: LocalizationIssueKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalizationIssueKind {
    MissingKey,
    OrphanKey,
}

impl LocalizationCatalog {
    pub fn new(default_locale: impl Into<String>) -> Self {
        Self {
            default_locale: default_locale.into(),
            locales: BTreeMap::new(),
        }
    }

    pub fn insert_locale_table(
        &mut self,
        locale: impl Into<String>,
        entries: BTreeMap<String, String>,
    ) {
        let locale = locale.into();
        self.locales.insert(locale, entries);
    }

    pub fn locale_codes(&self) -> Vec<String> {
        self.locales.keys().cloned().collect()
    }

    pub fn resolve<'a>(&'a self, locale: &str, key: &str) -> Option<&'a str> {
        self.locales
            .get(locale)
            .and_then(|table| table.get(key))
            .or_else(|| {
                self.locales
                    .get(self.default_locale.as_str())
                    .and_then(|table| table.get(key))
            })
            .map(std::string::String::as_str)
    }

    pub fn resolve_or_key(&self, locale: &str, key: &str) -> String {
        self.resolve(locale, key).unwrap_or(key).to_string()
    }

    pub fn validate_keys<'a, I>(&self, required_keys: I) -> Vec<LocalizationIssue>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let required: BTreeSet<String> = required_keys
            .into_iter()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        let mut issues = Vec::new();

        for (locale, table) in &self.locales {
            for key in &required {
                if !table.contains_key(key) {
                    issues.push(LocalizationIssue {
                        locale: locale.clone(),
                        key: key.clone(),
                        kind: LocalizationIssueKind::MissingKey,
                    });
                }
            }

            for key in table.keys() {
                if !required.contains(key) {
                    issues.push(LocalizationIssue {
                        locale: locale.clone(),
                        key: key.clone(),
                        kind: LocalizationIssueKind::OrphanKey,
                    });
                }
            }
        }

        issues
    }
}

pub fn localization_key(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    trimmed.strip_prefix(LOC_PREFIX).map(str::trim)
}

pub fn collect_script_localization_keys(script: &ScriptRaw) -> BTreeSet<String> {
    let mut out = BTreeSet::new();

    for event in &script.events {
        match event {
            EventRaw::Dialogue(d) => {
                if let Some(key) = localization_key(&d.speaker) {
                    if !key.is_empty() {
                        out.insert(key.to_string());
                    }
                }
                if let Some(key) = localization_key(&d.text) {
                    if !key.is_empty() {
                        out.insert(key.to_string());
                    }
                }
            }
            EventRaw::Choice(choice) => {
                if let Some(key) = localization_key(&choice.prompt) {
                    if !key.is_empty() {
                        out.insert(key.to_string());
                    }
                }
                for option in &choice.options {
                    if let Some(key) = localization_key(&option.text) {
                        if !key.is_empty() {
                            out.insert(key.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{ChoiceOptionRaw, ChoiceRaw, DialogueRaw};

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
}
