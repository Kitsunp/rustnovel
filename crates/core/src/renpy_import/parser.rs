use std::fs;
use std::path::Path;

use crate::error::{VnError, VnResult};
use crate::event::EventRaw;

use super::syntax::{
    parse_assignment_decl, parse_call_decl, parse_define_character, parse_dialogue_line,
    parse_elif_decl, parse_hide_decl, parse_image_alias, parse_jump_decl, parse_play_decl,
    parse_queue_decl, parse_scene_decl, parse_show_decl, parse_stop_decl, parse_with_decl,
    AssignmentValue,
};
use super::types::{ImportState, ParsedLine};

pub(super) fn parse_file(state: &mut ImportState, file: &Path) -> VnResult<()> {
    let raw = fs::read_to_string(file).map_err(|e| {
        VnError::InvalidScript(format!("renpy import: read {}: {e}", file.display()))
    })?;
    let lines = preprocess_lines(file, &raw);
    let mut idx = 0usize;
    state.parse_block(&lines, &mut idx, 0, lines.len());
    Ok(())
}

impl ImportState {
    pub(super) fn parse_block(
        &mut self,
        lines: &[ParsedLine],
        idx: &mut usize,
        base_indent: usize,
        limit: usize,
    ) {
        while *idx < limit {
            let Some(line) = lines.get(*idx) else {
                break;
            };
            if line.indent < base_indent {
                break;
            }
            if line.indent > base_indent {
                self.push_issue(
                    "warning",
                    "unexpected_indent",
                    "Ignoring line with unexpected indentation level",
                    Some(line),
                    None,
                );
                *idx = idx.saturating_add(1);
                continue;
            }
            self.parse_statement(lines, idx, base_indent, limit);
        }
    }

    fn parse_statement(
        &mut self,
        lines: &[ParsedLine],
        idx: &mut usize,
        base_indent: usize,
        limit: usize,
    ) {
        let Some(line) = lines.get(*idx).cloned() else {
            return;
        };
        let text = line.text.trim();

        if let Some(label_name) = parse_label_decl(text) {
            let resolved = self.resolve_label_name(&label_name);
            if !label_name.starts_with('.') {
                self.current_global_label = Some(resolved.clone());
            }
            self.add_label(&resolved);
            *idx = idx.saturating_add(1);
            if let Some(child_indent) = child_indent_of(lines, *idx, limit, base_indent) {
                self.parse_block(lines, idx, child_indent, limit);
            }
            return;
        }

        if let Some((alias, display)) = parse_define_character(text) {
            self.char_aliases.insert(alias, display);
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some((alias, path)) = parse_image_alias(text) {
            self.image_aliases.insert(alias, path);
            *idx = idx.saturating_add(1);
            return;
        }

        if text.starts_with("menu:") || text == "menu" {
            self.parse_menu(lines, idx, base_indent, limit);
            return;
        }

        if text.starts_with("if ") && text.ends_with(':') {
            self.parse_if_chain(lines, idx, base_indent, limit);
            return;
        }

        if let Some(cond_text) = parse_elif_decl(text) {
            self.push_issue(
                "warning",
                "elif_without_if",
                format!("Unexpected elif outside if-chain: {cond_text}"),
                Some(&line),
                None,
            );
            *idx = idx.saturating_add(1);
            return;
        }

        if text == "else:" {
            self.push_issue(
                "warning",
                "else_without_if",
                "Unexpected else outside if-chain",
                Some(&line),
                None,
            );
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some(target) = parse_jump_decl(text) {
            let resolved = self.resolve_label_name(&target);
            self.events.push(EventRaw::Jump { target: resolved });
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some(call_target) = parse_call_decl(text) {
            self.push_ext_call(
                "renpy_call",
                vec![call_target],
                Some(&line),
                "unsupported_call",
                "Ren'Py call statement converted to ext_call",
            );
            *idx = idx.saturating_add(1);
            return;
        }

        if text == "return" || text.starts_with("return ") {
            self.push_ext_call(
                "renpy_return",
                vec![text.to_string()],
                Some(&line),
                "unsupported_return",
                "Ren'Py return statement converted to ext_call",
            );
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some((key, value)) = parse_assignment_decl(text) {
            match value {
                AssignmentValue::Bool(v) => self.events.push(EventRaw::SetFlag { key, value: v }),
                AssignmentValue::Int(v) => self.events.push(EventRaw::SetVar { key, value: v }),
                AssignmentValue::Unsupported(raw) => self.push_ext_call(
                    "renpy_assignment",
                    vec![raw],
                    Some(&line),
                    "unsupported_assignment",
                    "Assignment could not be mapped to bool/int set",
                ),
            }
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some(scene) = parse_scene_decl(text, &self.image_aliases) {
            self.events.push(EventRaw::Scene(scene.scene));
            if let Some(trans) = scene.transition {
                self.events.push(EventRaw::Transition(trans));
            }
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some(show) = parse_show_decl(text, &self.image_aliases) {
            self.events.push(EventRaw::Patch(show.patch));
            if let Some(trans) = show.transition {
                self.events.push(EventRaw::Transition(trans));
            }
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some(hide) = parse_hide_decl(text) {
            self.events.push(EventRaw::Patch(hide.patch));
            if let Some(trans) = hide.transition {
                self.events.push(EventRaw::Transition(trans));
            }
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some(audio) = parse_play_decl(text) {
            self.events.push(EventRaw::AudioAction(audio));
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some(stop) = parse_stop_decl(text) {
            self.events.push(EventRaw::AudioAction(stop));
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some(queue_line) = parse_queue_decl(text) {
            self.push_ext_call(
                "renpy_queue_audio",
                vec![queue_line],
                Some(&line),
                "unsupported_audio_queue",
                "Ren'Py queue audio converted to ext_call",
            );
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some(trans) = parse_with_decl(text) {
            self.events.push(EventRaw::Transition(trans));
            *idx = idx.saturating_add(1);
            return;
        }

        if let Some(dialogue) = parse_dialogue_line(text, &self.char_aliases) {
            self.events.push(EventRaw::Dialogue(dialogue));
            *idx = idx.saturating_add(1);
            return;
        }

        let next_idx = idx.saturating_add(1);
        if text.ends_with(':') && child_indent_of(lines, next_idx, limit, base_indent).is_some() {
            let block_end = find_block_end(lines, next_idx, base_indent, limit);
            self.push_ext_call(
                "renpy_unsupported_block",
                vec![text.to_string()],
                Some(&line),
                "unsupported_block_statement",
                "Unsupported Ren'Py block converted to a single ext_call",
            );
            *idx = block_end;
            return;
        }

        self.push_ext_call(
            "renpy_unsupported",
            vec![text.to_string()],
            Some(&line),
            "unsupported_statement",
            "Unsupported Ren'Py statement converted to ext_call",
        );
        *idx = idx.saturating_add(1);
    }
}

fn preprocess_lines(file: &Path, raw: &str) -> Vec<ParsedLine> {
    raw.lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let mut stripped = strip_inline_comment(line).trim_end().to_string();
            if idx == 0 {
                stripped = stripped.trim_start_matches('\u{feff}').to_string();
            }
            if stripped.trim().is_empty() {
                return None;
            }
            let indent = count_indent(&stripped);
            let text = stripped.trim_start().to_string();
            Some(ParsedLine {
                file: file.to_path_buf(),
                line_no: idx + 1,
                indent,
                text,
            })
        })
        .collect()
}

pub(super) fn child_indent_of(
    lines: &[ParsedLine],
    start: usize,
    limit: usize,
    base_indent: usize,
) -> Option<usize> {
    for line in lines.iter().take(limit).skip(start) {
        if line.indent > base_indent {
            return Some(line.indent);
        }
        if line.indent < base_indent {
            return None;
        }
    }
    None
}

pub(super) fn find_block_end(
    lines: &[ParsedLine],
    start: usize,
    parent_indent: usize,
    limit: usize,
) -> usize {
    let mut idx = start;
    while idx < limit {
        let Some(line) = lines.get(idx) else {
            break;
        };
        if line.indent <= parent_indent {
            break;
        }
        idx += 1;
    }
    idx
}

fn parse_label_decl(text: &str) -> Option<String> {
    if !text.starts_with("label ") || !text.ends_with(':') {
        return None;
    }
    Some(
        text["label ".len()..text.len().saturating_sub(1)]
            .trim()
            .to_string(),
    )
}

fn strip_inline_comment(input: &str) -> String {
    let mut out = String::new();
    let mut quote_delimiter: Option<char> = None;
    let mut escape = false;
    for ch in input.chars() {
        if escape {
            out.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' {
            out.push(ch);
            escape = true;
            continue;
        }
        if quote_delimiter.is_some() {
            if Some(ch) == quote_delimiter {
                quote_delimiter = None;
            }
            out.push(ch);
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote_delimiter = Some(ch);
            out.push(ch);
            continue;
        }
        if ch == '#' {
            break;
        }
        out.push(ch);
    }
    out
}

fn count_indent(line: &str) -> usize {
    let mut count = 0usize;
    for ch in line.chars() {
        match ch {
            ' ' => count += 1,
            '\t' => count += 4,
            _ => break,
        }
    }
    count
}
