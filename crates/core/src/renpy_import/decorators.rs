use serde::Serialize;

use crate::event::EventRaw;

use super::types::{ImportArea, ImportPhase, ParsedLine};

pub(super) const IMPORT_EXTCALL_COMMAND_V2: &str = "vn.import.renpy.ext_v2";

pub(super) struct ExtCallDecorationInput<'a> {
    pub source_command: &'a str,
    pub payload: Vec<String>,
    pub issue_code: &'a str,
    pub trace_id: &'a str,
    pub area: ImportArea,
    pub phase: ImportPhase,
    pub line: Option<&'a ParsedLine>,
    pub event_ip: usize,
    pub active_label: Option<&'a str>,
}

pub(super) trait TraceDecorator {
    fn decorate_ext_call(&self, input: ExtCallDecorationInput<'_>) -> EventRaw;
}

#[derive(Default)]
pub(super) struct GraphTraceDecorator;

impl TraceDecorator for GraphTraceDecorator {
    fn decorate_ext_call(&self, input: ExtCallDecorationInput<'_>) -> EventRaw {
        let envelope = ExtCallTraceEnvelope {
            schema: "vn.import.trace.extcall.v2",
            trace_id: input.trace_id.to_string(),
            issue_code: input.issue_code.to_string(),
            action_id: action_id_for_code(input.issue_code),
            source_command: input.source_command.to_string(),
            phase: input.phase.as_str().to_string(),
            area: input.area.as_str().to_string(),
            event_ip: input.event_ip as u32,
            active_label: input.active_label.map(str::to_string),
            file: input
                .line
                .map(|line| line.file.to_string_lossy().replace('\\', "/")),
            line: input.line.map(|line| line.line_no as u32),
            snippet: input.line.map(|line| line.text.clone()),
            payload: input.payload.clone(),
        };

        let mut args = vec![serialize_envelope(&envelope)];
        args.extend(input.payload);
        EventRaw::ExtCall {
            command: IMPORT_EXTCALL_COMMAND_V2.to_string(),
            args,
        }
    }
}

#[derive(Serialize)]
struct ExtCallTraceEnvelope {
    schema: &'static str,
    trace_id: String,
    issue_code: String,
    action_id: String,
    source_command: String,
    phase: String,
    area: String,
    event_ip: u32,
    active_label: Option<String>,
    file: Option<String>,
    line: Option<u32>,
    snippet: Option<String>,
    payload: Vec<String>,
}

fn serialize_envelope(envelope: &ExtCallTraceEnvelope) -> String {
    serde_json::to_string(envelope).unwrap_or_else(|_| {
        format!(
            "schema={};trace_id={};issue_code={};action_id={}",
            envelope.schema, envelope.trace_id, envelope.issue_code, envelope.action_id
        )
    })
}

fn action_id_for_code(code: &str) -> String {
    let code_lc = code.trim().to_ascii_lowercase();
    let domain = if code_lc.contains("audio") {
        "audio"
    } else if code_lc.contains("menu")
        || code_lc.contains("if")
        || code_lc.contains("jump")
        || code_lc.contains("call")
        || code_lc.contains("return")
        || code_lc.contains("label")
        || code_lc.contains("target")
    {
        "flow"
    } else if code_lc.contains("assign") || code_lc.contains("flag") || code_lc.contains("var") {
        "state"
    } else if code_lc.contains("asset") || code_lc.contains("path") {
        "assets"
    } else {
        "generic"
    };

    let mode = if code_lc.starts_with("unsupported_")
        || code_lc.starts_with("menu_")
        || code_lc.starts_with("if_")
    {
        "fallback"
    } else {
        "normalize"
    };

    format!("renpy.{domain}.{mode}.{code_lc}")
}
