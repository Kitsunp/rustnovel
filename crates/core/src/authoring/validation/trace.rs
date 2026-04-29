use serde_json::Value;

pub(super) struct ImportTraceContext {
    pub trace_id: String,
    pub issue_code: String,
    pub source_command: String,
    pub phase: String,
    pub area: String,
    pub event_ip: Option<u32>,
    pub snippet: Option<String>,
    pub blocked_by: String,
}

pub(super) fn parse_import_trace_context(args: &[String]) -> Option<ImportTraceContext> {
    let envelope_raw = args.first()?;
    if let Ok(parsed) = serde_json::from_str::<Value>(envelope_raw) {
        if parsed.get("schema").and_then(Value::as_str) != Some("vn.import.trace.extcall.v2") {
            return None;
        }
        let trace_id = parsed.get("trace_id").and_then(Value::as_str)?.to_string();
        let issue_code = parsed
            .get("issue_code")
            .and_then(Value::as_str)
            .unwrap_or("unknown_issue")
            .to_string();
        let source_command = parsed
            .get("source_command")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let phase = parsed
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let area = parsed
            .get("area")
            .and_then(Value::as_str)
            .unwrap_or("other")
            .to_string();
        let event_ip = parsed
            .get("event_ip")
            .and_then(Value::as_u64)
            .map(|value| value as u32);
        let snippet = parsed
            .get("snippet")
            .and_then(Value::as_str)
            .map(std::string::ToString::to_string);
        let active_label = parsed
            .get("active_label")
            .and_then(Value::as_str)
            .unwrap_or("<none>");
        let file = parsed
            .get("file")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let line = parsed
            .get("line")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".to_string());
        let blocked_by = match event_ip {
            Some(ip) => format!("{file}:{line} label={active_label} ip={ip}"),
            None => format!("{file}:{line} label={active_label}"),
        };
        return Some(ImportTraceContext {
            trace_id,
            issue_code,
            source_command,
            phase,
            area,
            event_ip,
            snippet,
            blocked_by,
        });
    }

    let mut trace_id = None;
    let mut issue_code = None;
    let mut source_command = None;
    for chunk in envelope_raw.split(';') {
        let Some((key, value)) = chunk.split_once('=') else {
            continue;
        };
        match key.trim() {
            "trace_id" => trace_id = Some(value.trim().to_string()),
            "issue_code" => issue_code = Some(value.trim().to_string()),
            "source_command" => source_command = Some(value.trim().to_string()),
            _ => {}
        }
    }

    trace_id.map(|trace_id| ImportTraceContext {
        trace_id,
        issue_code: issue_code.unwrap_or_else(|| "unknown_issue".to_string()),
        source_command: source_command.unwrap_or_else(|| "unknown".to_string()),
        phase: "unknown".to_string(),
        area: "other".to_string(),
        event_ip: None,
        snippet: None,
        blocked_by: "trace envelope (fallback format)".to_string(),
    })
}
