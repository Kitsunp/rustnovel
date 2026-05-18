use std::collections::BTreeMap;

use crate::authoring::LintCode;

use super::{FieldPath, SemanticValue, SemanticValueKind};

pub(super) fn field_path_metadata(field_path: Option<&FieldPath>) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    if let Some(path) = field_path {
        metadata.insert("field_path".to_string(), path.value.clone());
        metadata.insert("field_path_key".to_string(), path.stable_key());
    }
    metadata
}

pub(super) fn semantic_value_metadata(value: &SemanticValue) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    metadata.insert("value_kind".to_string(), value.kind.label().to_string());
    metadata.insert("actual_raw".to_string(), value.raw_value.clone());
    metadata.insert(
        "actual_normalized".to_string(),
        value.normalized_value.clone(),
    );
    metadata.insert("owner_path".to_string(), value.owner_path.value.clone());
    if let Some(operation_id) = &value.introduced_by_operation {
        metadata.insert("operation_id".to_string(), operation_id.clone());
    }
    metadata
}

pub(super) fn resolver_metadata(
    code: LintCode,
    semantic_values: &[SemanticValue],
) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    metadata.insert("rule_code".to_string(), code.label().to_string());
    metadata.insert(
        "resolver_kind".to_string(),
        semantic_values
            .first()
            .map(|value| resolver_kind(&value.kind).to_string())
            .unwrap_or_else(|| "graph".to_string()),
    );
    metadata.insert("resolver_inputs".to_string(), join_raw(semantic_values));
    metadata.insert(
        "resolver_inputs_normalized".to_string(),
        join_normalized(semantic_values),
    );
    metadata
}

pub(super) fn rule_metadata(
    code: LintCode,
    semantic_values: &[SemanticValue],
    failure_summary: &str,
) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    metadata.insert("rule_code".to_string(), code.label().to_string());
    metadata.insert("failure_summary".to_string(), failure_summary.to_string());
    if let Some(value) = semantic_values.first() {
        metadata.insert("actual_raw".to_string(), value.raw_value.clone());
        metadata.insert(
            "actual_normalized".to_string(),
            value.normalized_value.clone(),
        );
        metadata.insert("expected_kind".to_string(), value.kind.label().to_string());
    }
    metadata
}

pub(super) fn failure_metadata(failure_summary: &str) -> BTreeMap<String, String> {
    BTreeMap::from([("failure_summary".to_string(), failure_summary.to_string())])
}

pub(super) fn consequence_metadata(code: LintCode) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("rule_code".to_string(), code.label().to_string()),
        (
            "runtime_consequence".to_string(),
            "runtime_or_preview_may_fail_or_diverge".to_string(),
        ),
    ])
}

pub(super) fn fix_metadata(code: LintCode) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("rule_code".to_string(), code.label().to_string()),
        (
            "fix_source".to_string(),
            "manual_edit_or_quick_fix_catalog".to_string(),
        ),
    ])
}

fn join_raw(values: &[SemanticValue]) -> String {
    values
        .iter()
        .map(|value| value.raw_value.clone())
        .collect::<Vec<_>>()
        .join("|")
}

fn join_normalized(values: &[SemanticValue]) -> String {
    values
        .iter()
        .map(|value| value.normalized_value.clone())
        .collect::<Vec<_>>()
        .join("|")
}

fn resolver_kind(kind: &SemanticValueKind) -> &'static str {
    match kind {
        SemanticValueKind::LabelRef => "label_resolver",
        SemanticValueKind::AssetRef => "asset_resolver",
        SemanticValueKind::VariableRef => "variable_resolver",
        SemanticValueKind::CharacterRef => "character_resolver",
        SemanticValueKind::PluginRef => "plugin_resolver",
        SemanticValueKind::AudioChannelRef => "audio_channel_resolver",
        SemanticValueKind::TransitionKind => "transition_resolver",
        SemanticValueKind::Text => "text_rule",
        SemanticValueKind::Number => "number_rule",
    }
}
