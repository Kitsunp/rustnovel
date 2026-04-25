use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::decorators::{ExtCallDecorationInput, GraphTraceDecorator, TraceDecorator};
use crate::error::VnResult;
use crate::event::{CondRaw, EventRaw};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ImportProfile {
    StoryFirst,
    Full,
    Custom,
}

impl ImportProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StoryFirst => "story_first",
            Self::Full => "full",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFallbackPolicy {
    Strict,
    DegradeWithTrace,
}

impl ImportFallbackPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::DegradeWithTrace => "degrade_with_trace",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportArea {
    Story,
    Ui,
    Translation,
    Assets,
    Flow,
    Other,
}

impl ImportArea {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Story => "story",
            Self::Ui => "ui",
            Self::Translation => "translation",
            Self::Assets => "assets",
            Self::Flow => "flow",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportPhase {
    Scan,
    Parse,
    Postprocess,
    AssetRewrite,
}

impl ImportPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::Parse => "parse",
            Self::Postprocess => "postprocess",
            Self::AssetRewrite => "asset_rewrite",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportIssueScope {
    pub area: ImportArea,
    pub phase: ImportPhase,
    pub fallback_applied: Option<String>,
}

impl ImportIssueScope {
    pub fn new(area: ImportArea, phase: ImportPhase, fallback_applied: Option<String>) -> Self {
        Self {
            area,
            phase,
            fallback_applied,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportRenpyOptions {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
    pub entry_label: String,
    pub report_path: Option<PathBuf>,
    pub profile: ImportProfile,
    /// Optional override for translation files under `game/tl`.
    pub include_tl: Option<bool>,
    /// Optional override for UI definition files like `gui.rpy` / `screens.rpy`.
    pub include_ui: Option<bool>,
    /// Optional include glob patterns (relative path, `/` normalized).
    pub include_patterns: Vec<String>,
    /// Optional exclude glob patterns (relative path, `/` normalized).
    pub exclude_patterns: Vec<String>,
    /// Strict mode fails import if any degraded event was required.
    pub strict_mode: bool,
    /// Fallback behavior for unsupported statements.
    pub fallback_policy: ImportFallbackPolicy,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportIssue {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub area: String,
    pub phase: String,
    pub snippet: Option<String>,
    pub path_display: String,
    pub fallback_applied: Option<String>,
    pub trace_id: String,
    pub root_cause: String,
    pub how_to_fix: String,
    pub docs_ref: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub importer_version: String,
    pub profile: String,
    pub strict_mode: bool,
    pub fallback_policy: String,
    pub project_root: String,
    pub scan_root: String,
    pub output_root: String,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub files_scanned: usize,
    pub files_parsed: usize,
    pub events_generated: usize,
    pub labels_generated: usize,
    pub degraded_events: usize,
    pub issues_by_code: BTreeMap<String, usize>,
    pub issues_by_area: BTreeMap<String, usize>,
    pub issues: Vec<ImportIssue>,
}

#[derive(Debug, Clone)]
pub struct ParsedLine {
    pub file: PathBuf,
    pub line_no: usize,
    pub indent: usize,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct MenuOptionBlock {
    pub line: ParsedLine,
    pub text: String,
    pub cond: Option<CondRaw>,
    pub body_start: usize,
    pub body_end: usize,
}

#[derive(Default)]
pub struct ImportState {
    pub events: Vec<EventRaw>,
    pub labels: BTreeMap<String, usize>,
    pub issues: Vec<ImportIssue>,
    pub char_aliases: HashMap<String, String>,
    pub image_aliases: HashMap<String, String>,
    pub current_global_label: Option<String>,
    pub synthetic_id: usize,
    pub trace_seq: usize,
    pub degraded_events: usize,
}

impl ImportState {
    pub fn push_issue(
        &mut self,
        severity: &str,
        code: &str,
        message: impl Into<String>,
        line: Option<&ParsedLine>,
        fallback_applied: Option<String>,
    ) -> String {
        self.push_issue_with_scope(
            severity,
            code,
            message,
            line,
            ImportIssueScope::new(
                classify_area_from_line(line),
                ImportPhase::Parse,
                fallback_applied,
            ),
        )
    }

    pub fn push_issue_with_scope(
        &mut self,
        severity: &str,
        code: &str,
        message: impl Into<String>,
        line: Option<&ParsedLine>,
        scope: ImportIssueScope,
    ) -> String {
        let trace_id = next_trace_id(&mut self.trace_seq);
        let docs = compose_issue_docs(
            code,
            scope.area,
            scope.phase,
            scope.fallback_applied.as_deref(),
        );
        self.issues.push(ImportIssue {
            severity: severity.to_string(),
            code: code.to_string(),
            message: message.into(),
            file: line.map(|ln| normalize_display_path(&ln.file)),
            line: line.map(|ln| ln.line_no),
            column: None,
            area: scope.area.as_str().to_string(),
            phase: scope.phase.as_str().to_string(),
            snippet: line.map(|ln| trim_snippet(&ln.text)),
            path_display: line
                .map(|ln| normalize_display_path(&ln.file))
                .unwrap_or_else(|| "unknown".to_string()),
            fallback_applied: scope.fallback_applied,
            trace_id: trace_id.clone(),
            root_cause: docs.root_cause,
            how_to_fix: docs.how_to_fix,
            docs_ref: docs.docs_ref,
        });
        trace_id
    }

    pub fn push_ext_call(
        &mut self,
        command: &str,
        args: Vec<String>,
        line: Option<&ParsedLine>,
        code: &str,
        _message: impl Into<String>,
    ) {
        let area = classify_area_from_line(line);
        let phase = ImportPhase::Parse;
        self.degraded_events = self.degraded_events.saturating_add(1);
        let composed_message = format!(
            "Ren'Py import fallback '{code}' aplicado desde '{command}' en area='{}' phase='{}'",
            area.as_str(),
            phase.as_str()
        );
        let trace_id = self.push_issue_with_scope(
            "warning",
            code,
            composed_message,
            line,
            ImportIssueScope::new(area, phase, Some("event_raw.ext_call".to_string())),
        );
        let decorator = GraphTraceDecorator;
        let decorated_event = decorator.decorate_ext_call(ExtCallDecorationInput {
            source_command: command,
            payload: args,
            issue_code: code,
            trace_id: &trace_id,
            area,
            phase,
            line,
            event_ip: self.events.len(),
            active_label: self.current_global_label.as_deref(),
        });
        self.events.push(decorated_event);
    }

    pub fn next_synthetic_label(&mut self, prefix: &str) -> String {
        self.synthetic_id = self.synthetic_id.saturating_add(1);
        format!("__import_{}_{}", prefix, self.synthetic_id)
    }

    pub fn resolve_label_name(&self, raw: &str) -> String {
        let name = raw.trim();
        if !name.starts_with('.') {
            return name.to_string();
        }
        if let Some(global) = &self.current_global_label {
            format!("{global}{name}")
        } else {
            name.trim_start_matches('.').to_string()
        }
    }

    pub fn add_label(&mut self, name: &str) {
        self.labels.insert(name.to_string(), self.events.len());
    }

    pub fn parse_file(&mut self, file: &Path) -> VnResult<()> {
        super::parser::parse_file(self, file)
    }
}

pub(super) fn classify_area_from_file(file: &Path) -> ImportArea {
    let normalized = file
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    if normalized.contains("/game/tl/") || normalized.starts_with("game/tl/") {
        return ImportArea::Translation;
    }
    if normalized.ends_with("/game/gui.rpy")
        || normalized.ends_with("/game/screens.rpy")
        || normalized.ends_with("/game/options.rpy")
        || normalized.contains("/game/gui/")
        || normalized == "game/gui.rpy"
        || normalized == "game/screens.rpy"
        || normalized == "game/options.rpy"
    {
        return ImportArea::Ui;
    }
    ImportArea::Story
}

fn classify_area_from_line(line: Option<&ParsedLine>) -> ImportArea {
    line.map(|ln| classify_area_from_file(&ln.file))
        .unwrap_or(ImportArea::Other)
}

fn trim_snippet(input: &str) -> String {
    const MAX_CHARS: usize = 160;
    let mut out = input.trim().to_string();
    if out.chars().count() <= MAX_CHARS {
        return out;
    }
    out = out.chars().take(MAX_CHARS).collect::<String>();
    out.push_str("...");
    out
}

pub(super) struct IssueDocs {
    pub root_cause: String,
    pub how_to_fix: String,
    pub docs_ref: String,
}

pub(super) fn compose_issue_docs(
    code: &str,
    area: ImportArea,
    phase: ImportPhase,
    fallback_applied: Option<&str>,
) -> IssueDocs {
    let class = classify_issue_class(code);
    let root_cause = match class {
        IssueClass::UnsupportedMapping => format!(
            "El flujo fuente contiene una construccion no mapeable al contrato canonico (area={}, phase={}).",
            area.as_str(),
            phase.as_str()
        ),
        IssueClass::AssetPolicy => format!(
            "La referencia de asset no cumple la politica de import (area={}, phase={}).",
            area.as_str(),
            phase.as_str()
        ),
        IssueClass::FlowIntegrity => format!(
            "Se detecto una inconsistencia de flujo/targets durante import (area={}, phase={}).",
            area.as_str(),
            phase.as_str()
        ),
        IssueClass::ParseShape => format!(
            "La forma sintactica no pudo normalizarse con el parser actual (area={}, phase={}).",
            area.as_str(),
            phase.as_str()
        ),
        IssueClass::Generic => format!(
            "La importacion encontro una condicion no normalizada (area={}, phase={}).",
            area.as_str(),
            phase.as_str()
        ),
    };

    let how_to_fix = match (class, fallback_applied) {
        (IssueClass::UnsupportedMapping, Some(path)) => format!(
            "Ajusta el script al subset soportado o define una accion custom; fallback aplicado: {path}."
        ),
        (IssueClass::UnsupportedMapping, None) => {
            "Ajusta el statement al subset soportado o habilita una estrategia de fallback.".to_string()
        }
        (IssueClass::AssetPolicy, _) => {
            "Usa rutas relativas seguras dentro del proyecto y verifica existencia de archivos."
                .to_string()
        }
        (IssueClass::FlowIntegrity, _) => {
            "Repara labels/targets y valida que entry_label apunte a una ruta ejecutable.".to_string()
        }
        (IssueClass::ParseShape, _) => {
            "Revisa indentacion/sintaxis y simplifica bloques para el perfil activo.".to_string()
        }
        (IssueClass::Generic, _) => {
            "Inspecciona el reporte y aplica normalizacion segun el perfil de import.".to_string()
        }
    };

    let docs_ref = format!("docs/import/renpy/{}/{}", phase.as_str(), code);
    IssueDocs {
        root_cause,
        how_to_fix,
        docs_ref,
    }
}

#[derive(Clone, Copy)]
enum IssueClass {
    UnsupportedMapping,
    AssetPolicy,
    FlowIntegrity,
    ParseShape,
    Generic,
}

fn classify_issue_class(code: &str) -> IssueClass {
    if code.starts_with("unsupported_")
        || code.starts_with("menu_")
        || code.starts_with("if_")
        || code.starts_with("renpy_unsupported")
    {
        return IssueClass::UnsupportedMapping;
    }
    if code.starts_with("asset_") {
        return IssueClass::AssetPolicy;
    }
    if code.contains("label") || code.contains("target") || code.contains("entry") {
        return IssueClass::FlowIntegrity;
    }
    if code.contains("indent") || code.contains("parse") {
        return IssueClass::ParseShape;
    }
    IssueClass::Generic
}

pub(super) fn next_trace_id(seq: &mut usize) -> String {
    *seq = seq.saturating_add(1);
    format!("imp-{seq:08}")
}

fn normalize_display_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("//?/")
        .trim_start_matches("\\\\?/")
        .to_string()
}
