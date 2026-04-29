use super::{LintCode, LintIssue};

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
}

#[derive(Debug, Clone)]
pub struct DiagnosticExplanation {
    pub title: String,
    pub root_cause: String,
    pub why_failed: String,
    pub how_to_fix: String,
    pub docs_ref: String,
}

impl LintIssue {
    pub fn explanation(&self, language: DiagnosticLanguage) -> DiagnosticExplanation {
        let title = match language {
            DiagnosticLanguage::Es => title_es(self.code),
            DiagnosticLanguage::En => title_en(self.code),
        };
        let (root_cause, why_failed, how_to_fix) = match language {
            DiagnosticLanguage::Es => (
                "El grafo de autoria no cumple el contrato semantico.",
                "El contrato headless produciria preview/export inconsistente.",
                "Corrige el nodo o aplica un quick-fix semantico seguro.",
            ),
            DiagnosticLanguage::En => (
                "The authoring graph violates the semantic contract.",
                "The headless contract would produce inconsistent preview/export output.",
                "Fix the node or apply a safe semantic quick-fix.",
            ),
        };
        DiagnosticExplanation {
            title: title.to_string(),
            root_cause: root_cause.to_string(),
            why_failed: why_failed.to_string(),
            how_to_fix: how_to_fix.to_string(),
            docs_ref: "docs/phase10_production_plan.md#authoring-contract".to_string(),
        }
    }

    pub fn localized_message(&self, language: DiagnosticLanguage) -> String {
        let explanation = self.explanation(language);
        format!("{}: {}", explanation.title, self.message)
    }
}

fn title_es(code: LintCode) -> &'static str {
    match code {
        LintCode::MissingStart => "Falta nodo Start",
        LintCode::MultipleStart => "Multiples nodos Start",
        LintCode::UnreachableNode => "Nodo inalcanzable",
        LintCode::PotentialLoop => "Bucle potencial",
        LintCode::DeadEnd => "Nodo sin salida",
        LintCode::ChoiceNoOptions => "Choice sin opciones",
        LintCode::ChoiceOptionUnlinked => "Opcion sin conexion",
        LintCode::ChoicePortOutOfRange => "Puerto de Choice fuera de rango",
        LintCode::AudioAssetMissing | LintCode::AudioAssetEmpty => "Audio sin asset valido",
        LintCode::AssetReferenceMissing => "Referencia de asset inexistente",
        LintCode::SceneBackgroundEmpty => "Background vacio",
        LintCode::UnsafeAssetPath => "Ruta de asset insegura",
        LintCode::InvalidAudioChannel
        | LintCode::InvalidAudioAction
        | LintCode::InvalidAudioVolume
        | LintCode::InvalidAudioFade => "Parametros de audio invalidos",
        LintCode::InvalidCharacterScale | LintCode::EmptyCharacterName => {
            "Datos de personaje invalidos"
        }
        LintCode::InvalidTransitionDuration | LintCode::InvalidTransitionKind => {
            "Transicion invalida"
        }
        LintCode::EmptySpeakerName => "Speaker vacio",
        LintCode::EmptyJumpTarget => "Jump sin destino",
        LintCode::MissingJumpTarget => "Jump apunta a un destino inexistente",
        LintCode::EmptyStateKey => "Llave de estado vacia",
        LintCode::InvalidLayoutPosition => "Posicion visual invalida",
        LintCode::PlaceholderChoiceOption => "Opcion placeholder sin editar",
        LintCode::ContractUnsupportedExport | LintCode::GenericEventUnchecked => {
            "Contrato de exportacion incompleto"
        }
        LintCode::CompileError | LintCode::RuntimeInitError => "Error de compilacion/runtime",
        LintCode::DryRunUnreachableCompiled
        | LintCode::DryRunStepLimit
        | LintCode::DryRunRuntimeError
        | LintCode::DryRunParityMismatch
        | LintCode::DryRunFinished => "Diagnostico de dry run",
    }
}

fn title_en(code: LintCode) -> &'static str {
    match code {
        LintCode::MissingStart => "Missing Start node",
        LintCode::MultipleStart => "Multiple Start nodes",
        LintCode::UnreachableNode => "Unreachable node",
        LintCode::PotentialLoop => "Potential loop",
        LintCode::DeadEnd => "Dead-end node",
        LintCode::ChoiceNoOptions => "Choice without options",
        LintCode::ChoiceOptionUnlinked => "Unlinked Choice option",
        LintCode::ChoicePortOutOfRange => "Choice port out of range",
        LintCode::AudioAssetMissing | LintCode::AudioAssetEmpty => "Audio missing valid asset",
        LintCode::AssetReferenceMissing => "Missing asset reference",
        LintCode::SceneBackgroundEmpty => "Empty scene background",
        LintCode::UnsafeAssetPath => "Unsafe asset path",
        LintCode::InvalidAudioChannel
        | LintCode::InvalidAudioAction
        | LintCode::InvalidAudioVolume
        | LintCode::InvalidAudioFade => "Invalid audio parameters",
        LintCode::InvalidCharacterScale | LintCode::EmptyCharacterName => "Invalid character data",
        LintCode::InvalidTransitionDuration | LintCode::InvalidTransitionKind => {
            "Invalid transition"
        }
        LintCode::EmptySpeakerName => "Empty speaker",
        LintCode::EmptyJumpTarget => "Empty jump target",
        LintCode::MissingJumpTarget => "Jump target does not exist",
        LintCode::EmptyStateKey => "Empty state key",
        LintCode::InvalidLayoutPosition => "Invalid visual position",
        LintCode::PlaceholderChoiceOption => "Unedited placeholder option",
        LintCode::ContractUnsupportedExport | LintCode::GenericEventUnchecked => {
            "Incomplete export contract"
        }
        LintCode::CompileError | LintCode::RuntimeInitError => "Compile/runtime error",
        LintCode::DryRunUnreachableCompiled
        | LintCode::DryRunStepLimit
        | LintCode::DryRunRuntimeError
        | LintCode::DryRunParityMismatch
        | LintCode::DryRunFinished => "Dry-run diagnostic",
    }
}
