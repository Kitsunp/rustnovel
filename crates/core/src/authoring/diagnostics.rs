mod catalog;
mod trace;

use std::collections::BTreeMap;

use serde::Serialize;

use super::LintIssue;
pub use trace::{
    DiagnosticTarget, EvidenceTrace, FieldPath, SemanticValue, SemanticValueKind, TraceAtom,
    TraceAtomKind, TraceEdge, TraceRelation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLanguage {
    Es,
    En,
}

impl DiagnosticLanguage {
    pub fn label(self) -> &'static str {
        match self {
            DiagnosticLanguage::Es => "ES",
            DiagnosticLanguage::En => "EN",
        }
    }

    pub fn locale(self) -> &'static str {
        match self {
            DiagnosticLanguage::Es => "es",
            DiagnosticLanguage::En => "en",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticExplanation {
    pub title: String,
    pub what_happened: String,
    pub root_cause: String,
    pub why_failed: String,
    pub consequence: String,
    pub how_to_fix: String,
    pub action_steps: Vec<String>,
    pub expected: String,
    pub actual: String,
    pub docs_ref: String,
    pub message_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticLocation {
    pub node_id: Option<u32>,
    pub event_ip: Option<u32>,
    pub edge_from: Option<u32>,
    pub edge_to: Option<u32>,
    pub asset_path: Option<String>,
    pub blocked_by: Option<String>,
    pub field_path: Option<FieldPath>,
    pub target: Option<DiagnosticTarget>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticEnvelopeV2 {
    pub schema: String,
    pub diagnostic_id: String,
    pub rule_version: String,
    pub message_key: String,
    pub message_args: BTreeMap<String, String>,
    pub severity: String,
    pub phase: String,
    pub code: String,
    pub location: DiagnosticLocation,
    pub target: Option<DiagnosticTarget>,
    pub field_path: Option<FieldPath>,
    pub semantic_values: Vec<SemanticValue>,
    pub evidence_trace: Option<EvidenceTrace>,
    pub trace_id: Option<String>,
    pub docs_ref: String,
    pub text_es: DiagnosticExplanation,
    pub text_en: DiagnosticExplanation,
}

impl DiagnosticEnvelopeV2 {
    pub fn localized(&self, language: DiagnosticLanguage) -> &DiagnosticExplanation {
        match language {
            DiagnosticLanguage::Es => &self.text_es,
            DiagnosticLanguage::En => &self.text_en,
        }
    }
}

impl LintIssue {
    pub fn explanation(&self, language: DiagnosticLanguage) -> DiagnosticExplanation {
        let entry = catalog::entry(self.code, language);
        DiagnosticExplanation {
            title: entry.title.to_string(),
            what_happened: entry.what_happened.to_string(),
            root_cause: entry.root_cause.to_string(),
            why_failed: entry.why_failed.to_string(),
            consequence: entry.consequence.to_string(),
            how_to_fix: entry.how_to_fix.to_string(),
            action_steps: entry
                .action_steps
                .iter()
                .map(|step| (*step).to_string())
                .collect(),
            expected: entry.expected.to_string(),
            actual: self.message.clone(),
            docs_ref: catalog::docs_ref(self.code),
            message_key: catalog::message_key(self.code),
        }
    }

    pub fn localized_message(&self, language: DiagnosticLanguage) -> String {
        let explanation = self.explanation(language);
        format!("{}: {}", explanation.title, self.message)
    }

    pub fn envelope_v2(&self) -> DiagnosticEnvelopeV2 {
        let text_es = self.explanation(DiagnosticLanguage::Es);
        let text_en = self.explanation(DiagnosticLanguage::En);
        DiagnosticEnvelopeV2 {
            schema: "vnengine.diagnostic_envelope.v2".to_string(),
            diagnostic_id: self.diagnostic_id(),
            rule_version: "authoring-diagnostic-v2".to_string(),
            message_key: text_en.message_key.clone(),
            message_args: self.message_args(),
            severity: self.severity.label().to_string(),
            phase: self.phase.label().to_string(),
            code: self.code.label().to_string(),
            location: DiagnosticLocation {
                node_id: self.node_id,
                event_ip: self.event_ip,
                edge_from: self.edge_from,
                edge_to: self.edge_to,
                asset_path: self.asset_path.clone(),
                blocked_by: self.blocked_by.clone(),
                field_path: self.field_path.clone(),
                target: self.target.clone(),
            },
            target: self.target.clone(),
            field_path: self.field_path.clone(),
            semantic_values: self.semantic_values.clone(),
            evidence_trace: self.evidence_trace.clone(),
            trace_id: self
                .evidence_trace
                .as_ref()
                .map(|trace| trace.trace_id.clone()),
            docs_ref: text_en.docs_ref.clone(),
            text_es,
            text_en,
        }
    }

    fn message_args(&self) -> BTreeMap<String, String> {
        let mut args = BTreeMap::new();
        args.insert("message".to_string(), self.message.clone());
        if let Some(node_id) = self.node_id {
            args.insert("node_id".to_string(), node_id.to_string());
        }
        if let Some(event_ip) = self.event_ip {
            args.insert("event_ip".to_string(), event_ip.to_string());
        }
        if let Some(edge_from) = self.edge_from {
            args.insert("edge_from".to_string(), edge_from.to_string());
        }
        if let Some(edge_to) = self.edge_to {
            args.insert("edge_to".to_string(), edge_to.to_string());
        }
        if let Some(asset_path) = &self.asset_path {
            args.insert("asset_path".to_string(), asset_path.clone());
        }
        if let Some(blocked_by) = &self.blocked_by {
            args.insert("blocked_by".to_string(), blocked_by.clone());
        }
        if let Some(field_path) = &self.field_path {
            args.insert("field_path".to_string(), field_path.value.clone());
        }
        if let Some(target) = &self.target {
            args.insert("target".to_string(), target.stable_key());
        }
        if let Some(trace) = &self.evidence_trace {
            args.insert("trace_id".to_string(), trace.trace_id.clone());
        }
        for (idx, value) in self.semantic_values.iter().enumerate() {
            let prefix = format!("semantic_value_{idx}");
            args.insert(format!("{prefix}_kind"), value.kind.label().to_string());
            args.insert(format!("{prefix}_raw"), value.raw_value.clone());
            args.insert(
                format!("{prefix}_normalized"),
                value.normalized_value.clone(),
            );
            args.insert(
                format!("{prefix}_owner_path"),
                value.owner_path.value.clone(),
            );
        }
        args
    }
}
