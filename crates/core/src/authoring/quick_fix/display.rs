use super::QuickFixCandidate;

impl QuickFixCandidate {
    pub fn title(&self, language: super::super::DiagnosticLanguage) -> &'static str {
        match language {
            super::super::DiagnosticLanguage::Es => self.title_es,
            super::super::DiagnosticLanguage::En => self.title_en,
        }
    }

    pub fn preconditions(&self, language: super::super::DiagnosticLanguage) -> &'static str {
        match language {
            super::super::DiagnosticLanguage::Es => self.preconditions_es,
            super::super::DiagnosticLanguage::En => self.preconditions_en,
        }
    }

    pub fn postconditions(&self, language: super::super::DiagnosticLanguage) -> &'static str {
        match language {
            super::super::DiagnosticLanguage::Es => self.postconditions_es,
            super::super::DiagnosticLanguage::En => self.postconditions_en,
        }
    }
}
