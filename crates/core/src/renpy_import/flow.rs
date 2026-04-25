use crate::event::{ChoiceOptionRaw, ChoiceRaw, CondRaw, EventRaw};

use super::parser::{child_indent_of, find_block_end};
use super::syntax::{
    parse_cond_expr, parse_elif_decl, parse_if_cond_decl, parse_menu_caption_line,
    parse_menu_option_decl,
};
use super::types::{ImportState, MenuOptionBlock, ParsedLine};

impl ImportState {
    pub(super) fn parse_menu(
        &mut self,
        lines: &[ParsedLine],
        idx: &mut usize,
        base_indent: usize,
        limit: usize,
    ) {
        let Some(menu_line) = lines.get(*idx).cloned() else {
            return;
        };
        let mut prompt = "Choose".to_string();
        *idx = idx.saturating_add(1);

        let Some(option_indent) = child_indent_of(lines, *idx, limit, base_indent) else {
            self.push_ext_call(
                "renpy_menu_empty",
                vec![menu_line.text.clone()],
                Some(&menu_line),
                "menu_without_options",
                "Menu has no options",
            );
            return;
        };

        let mut option_blocks = Vec::new();
        while *idx < limit {
            let Some(line) = lines.get(*idx) else {
                break;
            };
            if line.indent < option_indent {
                break;
            }
            if line.indent > option_indent {
                self.push_issue(
                    "warning",
                    "menu_indent",
                    "Skipping unexpected indentation inside menu",
                    Some(line),
                    None,
                );
                *idx = idx.saturating_add(1);
                continue;
            }

            if line.text.trim_start().starts_with("set ") {
                self.push_ext_call(
                    "renpy_menu_set",
                    vec![line.text.clone()],
                    Some(line),
                    "unsupported_menu_set",
                    "Ren'Py menu set clause converted to ext_call",
                );
                *idx = idx.saturating_add(1);
                continue;
            }

            let Some((opt_text, cond)) = parse_menu_option_decl(&line.text) else {
                if let Some(caption) = parse_menu_caption_line(&line.text) {
                    prompt = caption;
                    *idx = idx.saturating_add(1);
                    continue;
                }
                self.push_issue(
                    "warning",
                    "menu_option_parse",
                    "Could not parse menu option declaration",
                    Some(line),
                    None,
                );
                *idx = idx.saturating_add(1);
                continue;
            };

            *idx = idx.saturating_add(1);
            let body_start = *idx;
            let body_end = find_block_end(lines, body_start, option_indent, limit);
            option_blocks.push(MenuOptionBlock {
                line: line.clone(),
                text: opt_text,
                cond,
                body_start,
                body_end,
            });
            *idx = body_end;
        }

        let mut options = Vec::new();
        let mut block_plan: Vec<(String, usize, usize, ParsedLine)> = Vec::new();
        for block in option_blocks {
            if block.cond.is_some() {
                self.push_ext_call(
                    "renpy_menu_conditional_option",
                    vec![block.line.text.clone()],
                    Some(&block.line),
                    "unsupported_menu_option_cond",
                    "Conditional menu option converted to ext_call",
                );
                continue;
            }
            let target = self.next_synthetic_label("menu");
            options.push(ChoiceOptionRaw {
                text: block.text,
                target: target.clone(),
            });
            block_plan.push((target, block.body_start, block.body_end, block.line));
        }

        if options.is_empty() {
            self.push_ext_call(
                "renpy_menu_unusable",
                vec![menu_line.text.clone()],
                Some(&menu_line),
                "menu_no_supported_options",
                "Menu had no supported options after normalization",
            );
            return;
        }

        self.events
            .push(EventRaw::Choice(ChoiceRaw { prompt, options }));

        for (target, start, end, opt_line) in block_plan {
            self.labels.insert(target, self.events.len());
            let mut inner_idx = start;
            let child_indent = child_indent_of(lines, start, end, option_indent);
            if let Some(child_indent) = child_indent {
                self.parse_block(lines, &mut inner_idx, child_indent, end);
            } else {
                self.push_issue(
                    "warning",
                    "menu_option_empty_block",
                    "Menu option has empty body block",
                    Some(&opt_line),
                    None,
                );
            }
        }
    }

    pub(super) fn parse_if_chain(
        &mut self,
        lines: &[ParsedLine],
        idx: &mut usize,
        base_indent: usize,
        limit: usize,
    ) {
        let Some(first_line) = lines.get(*idx).cloned() else {
            return;
        };
        let mut branches: Vec<(Option<CondRaw>, usize, usize, ParsedLine)> = Vec::new();

        let mut cursor = *idx;
        loop {
            let Some(branch_line) = lines.get(cursor) else {
                break;
            };
            if branch_line.indent != base_indent {
                break;
            }

            let cond_opt = if branch_line.text.starts_with("if ") {
                parse_if_cond_decl(&branch_line.text).and_then(|expr| parse_cond_expr(&expr))
            } else if branch_line.text.starts_with("elif ") {
                parse_elif_decl(&branch_line.text).and_then(|expr| parse_cond_expr(&expr))
            } else if branch_line.text == "else:" {
                Some(CondRaw::Flag {
                    key: "__import_else".to_string(),
                    is_set: true,
                })
            } else {
                break;
            };

            let branch_head = branch_line.clone();
            cursor = cursor.saturating_add(1);
            let body_start = cursor;
            let body_end = find_block_end(lines, body_start, base_indent, limit);
            if body_start == body_end {
                self.push_issue(
                    "warning",
                    "if_empty_block",
                    "If/elif/else branch has empty body",
                    Some(&branch_head),
                    None,
                );
            }
            branches.push((cond_opt, body_start, body_end, branch_head));
            cursor = body_end;

            let Some(next_line) = lines.get(cursor) else {
                break;
            };
            if next_line.indent != base_indent {
                break;
            }
            if !(next_line.text.starts_with("elif ") || next_line.text == "else:") {
                break;
            }
        }

        if branches.is_empty() {
            self.push_ext_call(
                "renpy_if_invalid",
                vec![first_line.text.clone()],
                Some(&first_line),
                "if_parse_error",
                "Could not parse if-chain; converted to ext_call",
            );
            *idx = idx.saturating_add(1);
            return;
        }

        let end_label = self.next_synthetic_label("ifend");
        let mut pending_next_label: Option<String> = None;

        for (branch_idx, (cond_opt, start, end, branch_line)) in branches.iter().enumerate() {
            if let Some(next_label) = pending_next_label.take() {
                self.labels.insert(next_label, self.events.len());
            }

            if branch_line.text == "else:" {
                let mut inner = *start;
                if let Some(child_indent) = child_indent_of(lines, *start, *end, base_indent) {
                    self.parse_block(lines, &mut inner, child_indent, *end);
                }
                self.events.push(EventRaw::Jump {
                    target: end_label.clone(),
                });
                continue;
            }

            let Some(cond) = cond_opt.as_ref() else {
                self.push_ext_call(
                    "renpy_if_cond_unsupported",
                    vec![branch_line.text.clone()],
                    Some(branch_line),
                    "unsupported_if_condition",
                    "Unsupported if/elif condition converted to ext_call",
                );
                continue;
            };

            let true_label = self.next_synthetic_label("iftrue");
            let next_label = if branch_idx + 1 < branches.len() {
                Some(self.next_synthetic_label("ifnext"))
            } else {
                None
            };

            self.events.push(EventRaw::JumpIf {
                cond: cond.clone(),
                target: true_label.clone(),
            });
            if let Some(next_label) = &next_label {
                self.events.push(EventRaw::Jump {
                    target: next_label.clone(),
                });
            } else {
                self.events.push(EventRaw::Jump {
                    target: end_label.clone(),
                });
            }

            self.labels.insert(true_label, self.events.len());
            let mut inner = *start;
            if let Some(child_indent) = child_indent_of(lines, *start, *end, base_indent) {
                self.parse_block(lines, &mut inner, child_indent, *end);
            }
            self.events.push(EventRaw::Jump {
                target: end_label.clone(),
            });
            pending_next_label = next_label;
        }

        if let Some(next_label) = pending_next_label {
            self.labels.insert(next_label, self.events.len());
        }
        self.labels.insert(end_label, self.events.len());
        *idx = cursor;
    }
}
