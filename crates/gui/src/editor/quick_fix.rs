use crate::editor::{DiagnosticLanguage, LintIssue, NodeGraph};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickFixRisk {
    Safe,
    Review,
}

impl QuickFixRisk {
    pub fn label(self) -> &'static str {
        match self {
            QuickFixRisk::Safe => "SAFE",
            QuickFixRisk::Review => "REVIEW",
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuickFixCandidate {
    pub fix_id: &'static str,
    pub title_es: &'static str,
    pub title_en: &'static str,
    pub preconditions_es: &'static str,
    pub preconditions_en: &'static str,
    pub postconditions_es: &'static str,
    pub postconditions_en: &'static str,
    pub risk: QuickFixRisk,
    pub structural: bool,
}

impl QuickFixCandidate {
    pub fn title(&self, language: DiagnosticLanguage) -> &'static str {
        match language {
            DiagnosticLanguage::Es => self.title_es,
            DiagnosticLanguage::En => self.title_en,
        }
    }

    pub fn preconditions(&self, language: DiagnosticLanguage) -> &'static str {
        match language {
            DiagnosticLanguage::Es => self.preconditions_es,
            DiagnosticLanguage::En => self.preconditions_en,
        }
    }

    pub fn postconditions(&self, language: DiagnosticLanguage) -> &'static str {
        match language {
            DiagnosticLanguage::Es => self.postconditions_es,
            DiagnosticLanguage::En => self.postconditions_en,
        }
    }
}

mod catalog;

pub fn suggest_fixes(issue: &LintIssue, graph: &NodeGraph) -> Vec<QuickFixCandidate> {
    catalog::suggest_fixes(issue, graph)
}

pub fn apply_fix(graph: &mut NodeGraph, issue: &LintIssue, fix_id: &str) -> Result<bool, String> {
    catalog::apply_fix(graph, issue, fix_id)
}

#[cfg(test)]
#[path = "tests/quick_fix_tests.rs"]
mod tests;
