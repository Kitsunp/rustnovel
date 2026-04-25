use super::validator::{LintCode, LintIssue};

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

#[derive(Debug, Clone, Copy)]
struct DiagnosticCatalogEntry {
    title_es: &'static str,
    title_en: &'static str,
    root_cause_es: &'static str,
    root_cause_en: &'static str,
    why_failed_es: &'static str,
    why_failed_en: &'static str,
    how_to_fix_es: &'static str,
    how_to_fix_en: &'static str,
    docs_ref: &'static str,
}

impl DiagnosticCatalogEntry {
    fn text(self, language: DiagnosticLanguage) -> DiagnosticExplanation {
        match language {
            DiagnosticLanguage::Es => DiagnosticExplanation {
                title: self.title_es.to_string(),
                root_cause: self.root_cause_es.to_string(),
                why_failed: self.why_failed_es.to_string(),
                how_to_fix: self.how_to_fix_es.to_string(),
                docs_ref: self.docs_ref.to_string(),
            },
            DiagnosticLanguage::En => DiagnosticExplanation {
                title: self.title_en.to_string(),
                root_cause: self.root_cause_en.to_string(),
                why_failed: self.why_failed_en.to_string(),
                how_to_fix: self.how_to_fix_en.to_string(),
                docs_ref: self.docs_ref.to_string(),
            },
        }
    }
}

fn catalog_for(code: LintCode) -> DiagnosticCatalogEntry {
    match code {
        LintCode::MissingStart => DiagnosticCatalogEntry {
            title_es: "Falta nodo Start",
            title_en: "Missing Start node",
            root_cause_es: "El grafo no define un punto de entrada.",
            root_cause_en: "The graph has no explicit entry point.",
            why_failed_es: "Sin Start, la compilacion y ejecucion no tienen IP inicial valida.",
            why_failed_en: "Without Start, compile/runtime cannot resolve a valid initial IP.",
            how_to_fix_es: "Agrega un nodo Start y conecta su salida al primer nodo de flujo.",
            how_to_fix_en: "Add a Start node and connect its output to the first flow node.",
            docs_ref: "docs/phase10_production_plan.md#101-contratos-de-datos--migraciones-reales",
        },
        LintCode::MultipleStart => DiagnosticCatalogEntry {
            title_es: "Multiples nodos Start",
            title_en: "Multiple Start nodes",
            root_cause_es: "Hay mas de un punto de entrada declarado.",
            root_cause_en: "More than one entry point is declared.",
            why_failed_es: "La semantica de arranque se vuelve ambigua para preview/runtime/export.",
            why_failed_en: "Startup semantics become ambiguous across preview/runtime/export.",
            how_to_fix_es: "Conserva un solo Start y redirige o elimina los restantes.",
            how_to_fix_en: "Keep a single Start and redirect or remove the rest.",
            docs_ref: "docs/phase10_production_plan.md#101-contratos-de-datos--migraciones-reales",
        },
        LintCode::UnreachableNode => DiagnosticCatalogEntry {
            title_es: "Nodo inalcanzable",
            title_en: "Unreachable node",
            root_cause_es: "No existe camino desde ningun Start hacia ese nodo.",
            root_cause_en: "No path exists from any Start to that node.",
            why_failed_es: "La rama queda fuera de ejecucion y puede ocultar deuda o errores logicos.",
            why_failed_en: "The branch is never executed and may hide logical defects.",
            how_to_fix_es: "Conecta el nodo a una ruta valida o elimina codigo muerto.",
            how_to_fix_en: "Connect the node into a valid route or remove dead code.",
            docs_ref: "docs/phase10_production_plan.md#106-herramientas-de-autoria-avanzada",
        },
        LintCode::PotentialLoop => DiagnosticCatalogEntry {
            title_es: "Bucle potencial en ruta alcanzable",
            title_en: "Potential loop on reachable route",
            root_cause_es: "El grafo contiene un ciclo que puede recorrerse desde Start.",
            root_cause_en: "Graph contains a cycle reachable from Start.",
            why_failed_es: "El flujo puede no terminar y disparar limites de simulacion o bloqueo de avance.",
            why_failed_en: "Flow may never terminate and can trigger simulation limits or progression stalls.",
            how_to_fix_es: "Introduce condicion de salida, redirige una arista o agrega ruta de escape a End.",
            how_to_fix_en: "Add an exit condition, reroute one edge, or add an escape path to End.",
            docs_ref: "docs/phase10_production_plan.md#106-herramientas-de-autoria-avanzada",
        },
        LintCode::DeadEnd => DiagnosticCatalogEntry {
            title_es: "Nodo sin salida",
            title_en: "Dead-end node",
            root_cause_es: "El nodo no tiene transicion saliente.",
            root_cause_en: "The node has no outgoing transition.",
            why_failed_es: "La ejecucion puede quedar bloqueada sin cierre narrativo controlado.",
            why_failed_en: "Execution may stall without a controlled narrative closure.",
            how_to_fix_es: "Conecta el nodo a otro nodo o a un End explicito.",
            how_to_fix_en: "Connect the node to another node or to an explicit End node.",
            docs_ref: "docs/phase10_production_plan.md#103-componentes-vn-esenciales-faltantes",
        },
        LintCode::ChoiceNoOptions => DiagnosticCatalogEntry {
            title_es: "Choice sin opciones",
            title_en: "Choice without options",
            root_cause_es: "El nodo Choice no define alternativas.",
            root_cause_en: "The Choice node declares no alternatives.",
            why_failed_es: "No hay decision jugable y la ruta no puede avanzar correctamente.",
            why_failed_en: "There is no playable decision and route progress becomes invalid.",
            how_to_fix_es: "Agrega opciones y conecta cada salida a un destino.",
            how_to_fix_en: "Add options and connect each output to a destination.",
            docs_ref: "docs/phase10_production_plan.md#103-componentes-vn-esenciales-faltantes",
        },
        LintCode::ChoiceOptionUnlinked => DiagnosticCatalogEntry {
            title_es: "Opcion de Choice sin conexion",
            title_en: "Unlinked Choice option",
            root_cause_es: "Una opcion no tiene edge de salida asociado.",
            root_cause_en: "An option has no outgoing edge.",
            why_failed_es: "La opcion seleccionable no tiene destino y rompe continuidad narrativa.",
            why_failed_en: "The selectable option has no target and breaks narrative continuity.",
            how_to_fix_es: "Conecta la opcion faltante a un nodo destino valido.",
            how_to_fix_en: "Connect the missing option to a valid target node.",
            docs_ref: "docs/phase10_production_plan.md#106-herramientas-de-autoria-avanzada",
        },
        LintCode::ChoicePortOutOfRange => DiagnosticCatalogEntry {
            title_es: "Puerto de Choice fuera de rango",
            title_en: "Choice port out of range",
            root_cause_es: "Existe una conexion en un puerto que no corresponde a una opcion.",
            root_cause_en: "A connection references a port with no matching option.",
            why_failed_es: "El contrato nodo->puerto->opcion queda inconsistente.",
            why_failed_en: "The node->port->option contract becomes inconsistent.",
            how_to_fix_es: "Sincroniza cantidad de opciones y puertos, o remueve la conexion invalida.",
            how_to_fix_en: "Synchronize option count and ports, or remove the invalid connection.",
            docs_ref: "docs/phase10_production_plan.md#101-contratos-de-datos--migraciones-reales",
        },
        LintCode::AudioAssetMissing | LintCode::AudioAssetEmpty => DiagnosticCatalogEntry {
            title_es: "Audio sin asset valido",
            title_en: "Audio missing valid asset",
            root_cause_es: "La accion de audio requiere ruta de asset y no existe o esta vacia.",
            root_cause_en: "Audio action requires an asset path and it is missing/empty.",
            why_failed_es: "En runtime la orden de audio no puede resolverse a recurso reproducible.",
            why_failed_en: "Runtime cannot resolve the audio command to a playable resource.",
            how_to_fix_es: "Define una ruta de audio valida o ajusta la accion si no requiere asset.",
            how_to_fix_en: "Provide a valid audio path or adjust action when asset is not required.",
            docs_ref: "docs/phase10_production_plan.md#105-pipeline-de-assets-de-produccion",
        },
        LintCode::AssetReferenceMissing => DiagnosticCatalogEntry {
            title_es: "Referencia de asset inexistente",
            title_en: "Missing asset reference",
            root_cause_es: "La ruta apunta a un recurso que no existe en disco/proyecto.",
            root_cause_en: "The path points to a resource that does not exist.",
            why_failed_es: "La carga de escena/audio falla por referencia rota.",
            why_failed_en: "Scene/audio loading fails due to a broken reference.",
            how_to_fix_es: "Corrige la ruta o importa el asset faltante.",
            how_to_fix_en: "Fix the path or import the missing asset.",
            docs_ref: "docs/phase10_production_plan.md#105-pipeline-de-assets-de-produccion",
        },
        LintCode::SceneBackgroundEmpty => DiagnosticCatalogEntry {
            title_es: "Background de escena vacio",
            title_en: "Empty scene background",
            root_cause_es: "El campo background se declara pero sin valor util.",
            root_cause_en: "Background field is declared with an empty value.",
            why_failed_es: "La escena queda con estado ambiguo de imagen.",
            why_failed_en: "Scene image state becomes ambiguous.",
            how_to_fix_es: "Elimina el campo vacio o asigna una ruta valida.",
            how_to_fix_en: "Remove the empty field or set a valid path.",
            docs_ref: "docs/phase10_production_plan.md#103-componentes-vn-esenciales-faltantes",
        },
        LintCode::UnsafeAssetPath => DiagnosticCatalogEntry {
            title_es: "Ruta de asset insegura",
            title_en: "Unsafe asset path",
            root_cause_es: "La ruta contiene patrones bloqueados (traversal, absoluto o URL).",
            root_cause_en: "Path contains blocked patterns (traversal, absolute, or URL).",
            why_failed_es: "Viola politicas de seguridad de referencia de recursos.",
            why_failed_en: "It violates resource path security policy.",
            how_to_fix_es: "Usa una ruta relativa saneada dentro del proyecto.",
            how_to_fix_en: "Use a sanitized relative path inside the project.",
            docs_ref: "docs/integrity_threat_model.md",
        },
        LintCode::InvalidAudioChannel
        | LintCode::InvalidAudioAction
        | LintCode::InvalidAudioVolume
        | LintCode::InvalidAudioFade => DiagnosticCatalogEntry {
            title_es: "Parametros de audio invalidos",
            title_en: "Invalid audio parameters",
            root_cause_es: "Uno o mas parametros de audio no cumplen el contrato.",
            root_cause_en: "One or more audio parameters violate the contract.",
            why_failed_es: "El comando de audio puede degradarse o comportarse distinto en runtime.",
            why_failed_en: "Audio command may degrade or behave differently at runtime.",
            how_to_fix_es: "Normaliza canal/accion/volumen/fade a valores soportados.",
            how_to_fix_en: "Normalize channel/action/volume/fade to supported values.",
            docs_ref: "docs/phase10_production_plan.md#103-componentes-vn-esenciales-faltantes",
        },
        LintCode::InvalidCharacterScale | LintCode::EmptyCharacterName => DiagnosticCatalogEntry {
            title_es: "Datos de personaje invalidos",
            title_en: "Invalid character data",
            root_cause_es: "Nombre o escala del personaje no cumple restricciones.",
            root_cause_en: "Character name or scale violates constraints.",
            why_failed_es: "La composicion visual puede fallar o quedar inconsistente.",
            why_failed_en: "Visual composition may fail or become inconsistent.",
            how_to_fix_es: "Corrige nombre y escala en entradas de personaje.",
            how_to_fix_en: "Fix name and scale in character entries.",
            docs_ref: "docs/phase10_production_plan.md#103-componentes-vn-esenciales-faltantes",
        },
        LintCode::InvalidTransitionDuration | LintCode::InvalidTransitionKind => {
            DiagnosticCatalogEntry {
                title_es: "Transicion invalida",
                title_en: "Invalid transition",
                root_cause_es: "Duracion o tipo de transicion no esta en contrato.",
                root_cause_en: "Transition duration or kind is outside contract.",
                why_failed_es: "La transicion puede degradar a fallback no deseado.",
                why_failed_en: "Transition may degrade to an unintended fallback.",
                how_to_fix_es: "Usa tipo soportado y duracion mayor a cero.",
                how_to_fix_en: "Use a supported kind and duration greater than zero.",
                docs_ref: "docs/phase10_production_plan.md#103-componentes-vn-esenciales-faltantes",
            }
        }
        LintCode::EmptySpeakerName => DiagnosticCatalogEntry {
            title_es: "Dialogo sin speaker",
            title_en: "Dialogue without speaker",
            root_cause_es: "El nodo Dialogue tiene nombre de speaker vacio.",
            root_cause_en: "Dialogue node has an empty speaker name.",
            why_failed_es: "El historial y la UI pierden contexto de quien habla.",
            why_failed_en: "History and UI lose speaker context.",
            how_to_fix_es: "Define speaker valido o un narrador explicito.",
            how_to_fix_en: "Set a valid speaker or an explicit narrator.",
            docs_ref: "docs/phase10_production_plan.md#103-componentes-vn-esenciales-faltantes",
        },
        LintCode::EmptyJumpTarget => DiagnosticCatalogEntry {
            title_es: "Salto sin target",
            title_en: "Jump without target",
            root_cause_es: "Jump/JumpIf no tiene label de destino.",
            root_cause_en: "Jump/JumpIf has no destination label.",
            why_failed_es: "La navegacion no puede resolver el proximo IP.",
            why_failed_en: "Navigation cannot resolve the next instruction pointer.",
            how_to_fix_es: "Asigna label valido (por ejemplo start o nodo de destino).",
            how_to_fix_en: "Assign a valid label (for example start or a target node label).",
            docs_ref: "docs/phase10_production_plan.md#101-contratos-de-datos--migraciones-reales",
        },
        LintCode::ContractUnsupportedExport => DiagnosticCatalogEntry {
            title_es: "Evento no exportable por contrato",
            title_en: "Contract-unsupported export event",
            root_cause_es: "El nodo no pertenece al conjunto runtime-real exportable.",
            root_cause_en: "Node is not part of runtime-real exportable contract.",
            why_failed_es: "Preview y export quedarian con semantica divergente.",
            why_failed_en: "Preview and export semantics would diverge.",
            how_to_fix_es: "Reemplaza por eventos soportados para export o marca preview-only.",
            how_to_fix_en: "Replace with export-supported events or keep preview-only.",
            docs_ref: "docs/phase10_production_plan.md#101-contratos-de-datos--migraciones-reales",
        },
        LintCode::GenericEventUnchecked => DiagnosticCatalogEntry {
            title_es: "Evento generico con validacion parcial",
            title_en: "Generic event with partial validation",
            root_cause_es: "El editor no conoce semantica completa del evento generico.",
            root_cause_en: "Editor does not know full semantics of generic event.",
            why_failed_es: "La validez total depende del runtime y puede ser incierta.",
            why_failed_en: "Full validity depends on runtime and may be uncertain.",
            how_to_fix_es: "Migra a nodo tipado soportado o valida manualmente en dry run.",
            how_to_fix_en: "Migrate to a supported typed node or validate manually via dry run.",
            docs_ref: "docs/phase10_production_plan.md#101-contratos-de-datos--migraciones-reales",
        },
        LintCode::CompileError => DiagnosticCatalogEntry {
            title_es: "Error de compilacion de script",
            title_en: "Script compilation error",
            root_cause_es: "El ScriptRaw no puede compilarse a representacion runtime.",
            root_cause_en: "ScriptRaw cannot compile into runtime representation.",
            why_failed_es: "Hay inconsistencia semantica en labels, targets o eventos.",
            why_failed_en: "There is a semantic inconsistency in labels, targets, or events.",
            how_to_fix_es: "Corrige diagnosticos de compile y vuelve a validar.",
            how_to_fix_en: "Fix compile diagnostics and validate again.",
            docs_ref: "docs/phase10_production_plan.md#107-observabilidad-operativa--diagnostico-reproducible",
        },
        LintCode::RuntimeInitError => DiagnosticCatalogEntry {
            title_es: "Fallo de inicializacion runtime",
            title_en: "Runtime initialization failure",
            root_cause_es: "Engine no pudo inicializar con el script compilado.",
            root_cause_en: "Engine could not initialize with the compiled script.",
            why_failed_es: "El estado inicial no cumple precondiciones de ejecucion.",
            why_failed_en: "Initial state violates execution preconditions.",
            how_to_fix_es: "Revisa errores previos de compile/dry run y contratos de recursos.",
            how_to_fix_en: "Review earlier compile/dry-run errors and resource contracts.",
            docs_ref: "docs/phase10_production_plan.md#107-observabilidad-operativa--diagnostico-reproducible",
        },
        LintCode::DryRunUnreachableCompiled
        | LintCode::DryRunStepLimit
        | LintCode::DryRunRuntimeError
        | LintCode::DryRunParityMismatch
        | LintCode::DryRunFinished => DiagnosticCatalogEntry {
            title_es: "Diagnostico de dry run/paridad",
            title_en: "Dry-run/parity diagnostic",
            root_cause_es: "La simulacion detecto divergencia, limite o error de ruta.",
            root_cause_en: "Simulation detected divergence, route limit, or runtime error.",
            why_failed_es: "Preview y runtime pueden no coincidir en una o mas rutas.",
            why_failed_en: "Preview and runtime may diverge on one or more routes.",
            how_to_fix_es: "Usa el repro exportable, corrige ruta y vuelve a ejecutar dry run.",
            how_to_fix_en: "Use exported repro, fix route behavior, and run dry run again.",
            docs_ref: "docs/phase10_production_plan.md#107-observabilidad-operativa--diagnostico-reproducible",
        },
    }
}

impl LintIssue {
    pub fn explanation(&self, language: DiagnosticLanguage) -> DiagnosticExplanation {
        let mut explanation = catalog_for(self.code).text(language);
        let mut parts = Vec::new();
        if let Some(node_id) = self.node_id {
            parts.push(format!("node_id={node_id}"));
        }
        if let Some(event_ip) = self.event_ip {
            parts.push(format!("event_ip={event_ip}"));
        }
        if let (Some(edge_from), Some(edge_to)) = (self.edge_from, self.edge_to) {
            parts.push(format!("edge={edge_from}->{edge_to}"));
        } else if let Some(edge_from) = self.edge_from {
            parts.push(format!("edge_from={edge_from}"));
        }
        if let Some(asset_path) = &self.asset_path {
            parts.push(format!("asset={asset_path}"));
        }
        if let Some(blocked_by) = &self.blocked_by {
            parts.push(format!("blocked_by={blocked_by}"));
        }
        let context = if parts.is_empty() {
            String::new()
        } else {
            format!(" {}", parts.join(", "))
        };
        if !context.is_empty() {
            explanation.why_failed = format!("{} | Context:{}", explanation.why_failed, context);
        }
        explanation
    }

    pub fn localized_title(&self, language: DiagnosticLanguage) -> String {
        self.explanation(language).title
    }

    pub fn localized_message(&self, language: DiagnosticLanguage) -> String {
        let title = self.localized_title(language);
        if self.message.trim().is_empty() {
            title
        } else {
            format!("{title}: {}", self.message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::validator::{LintCode, ValidationPhase};

    #[test]
    fn explanation_is_available_in_es_and_en() {
        let issue = LintIssue::error(
            Some(7),
            ValidationPhase::Graph,
            LintCode::ChoiceNoOptions,
            "choice node has no options",
        )
        .with_event_ip(Some(3));

        let es = issue.explanation(DiagnosticLanguage::Es);
        let en = issue.explanation(DiagnosticLanguage::En);

        assert!(!es.root_cause.is_empty());
        assert!(!es.why_failed.is_empty());
        assert!(!es.how_to_fix.is_empty());
        assert!(!en.root_cause.is_empty());
        assert!(!en.why_failed.is_empty());
        assert!(!en.how_to_fix.is_empty());
        assert!(en.docs_ref.starts_with("docs/"));
        assert!(es.why_failed.contains("Context:"));
        assert!(en.why_failed.contains("Context:"));
    }

    #[test]
    fn localized_message_keeps_title_and_runtime_message() {
        let issue = LintIssue::warning(
            None,
            ValidationPhase::Graph,
            LintCode::EmptySpeakerName,
            "Dialogue speaker is empty",
        );
        let es = issue.localized_message(DiagnosticLanguage::Es);
        let en = issue.localized_message(DiagnosticLanguage::En);

        assert!(es.contains("Dialogue speaker is empty"));
        assert!(en.contains("Dialogue speaker is empty"));
    }
}
