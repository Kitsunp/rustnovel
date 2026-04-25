//! Save Preview / Diff Dialog
//!
//! Visualizes changes before saving script.

use eframe::egui;
use visual_novel_engine::ScriptRaw;

pub struct DiffDialog {
    previous_script: Option<ScriptRaw>,
    current_script: ScriptRaw,
    stats: DiffStats,
    lines: Vec<DiffRow>,
    title: String,
    intro_text: String,
    warning_text: String,
    confirm_label: String,
    cancel_label: String,
}

#[derive(Clone, Debug, Default)]
struct DiffStats {
    added_events: usize,
    removed_events: usize,
    modified_events: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiffKind {
    Added,
    Removed,
    Modified,
    Context,
    Elided,
}

#[derive(Clone, Debug)]
struct DiffRow {
    kind: DiffKind,
    left_no: Option<usize>,
    right_no: Option<usize>,
    left_text: String,
    right_text: String,
}

impl DiffDialog {
    pub fn new(previous_script: Option<&ScriptRaw>, current_script: &ScriptRaw) -> Self {
        Self::new_save(previous_script, current_script)
    }

    pub fn new_save(previous_script: Option<&ScriptRaw>, current_script: &ScriptRaw) -> Self {
        Self::new_with_context(
            previous_script,
            current_script,
            "Confirmar Cambios".to_string(),
            "Estas por guardar cambios en el script.".to_string(),
            "Esto sobrescribira el archivo en disco.".to_string(),
            "Confirmar Guardado".to_string(),
            "Cancelar".to_string(),
        )
    }

    pub fn new_quick_fix(
        previous_script: Option<&ScriptRaw>,
        current_script: &ScriptRaw,
        fix_id: &str,
    ) -> Self {
        Self::new_with_context(
            previous_script,
            current_script,
            format!("Confirmar Quick-Fix Estructural ({fix_id})"),
            format!(
                "Este quick-fix estructural ('{fix_id}') modificara el grafo. Revisa el diff antes de aplicar."
            ),
            "Solo aplica si el cambio respeta tu intencion narrativa.".to_string(),
            "Aplicar Quick-Fix".to_string(),
            "Cancelar".to_string(),
        )
    }

    pub fn new_autofix_batch(
        previous_script: Option<&ScriptRaw>,
        current_script: &ScriptRaw,
        fix_count: usize,
        include_review: bool,
    ) -> Self {
        let mode = if include_review {
            "completo (safe + review)"
        } else {
            "safe"
        };
        Self::new_with_context(
            previous_script,
            current_script,
            format!("Confirmar Auto-Fix {mode}"),
            format!(
                "Se aplicaran {fix_count} fix(es) automáticos en modo {mode}. Revisa el diff horizontal antes de confirmar."
            ),
            "Confirma solo si el resultado conserva la semantica narrativa y de ejecucion."
                .to_string(),
            "Aplicar Auto-Fix".to_string(),
            "Cancelar".to_string(),
        )
    }

    fn new_with_context(
        previous_script: Option<&ScriptRaw>,
        current_script: &ScriptRaw,
        title: String,
        intro_text: String,
        warning_text: String,
        confirm_label: String,
        cancel_label: String,
    ) -> Self {
        let previous_script = previous_script.cloned();
        let current_script = current_script.clone();
        let stats = compute_stats(previous_script.as_ref(), &current_script);
        let lines = build_diff_rows(previous_script.as_ref(), &current_script);

        Self {
            previous_script,
            current_script,
            stats,
            lines,
            title,
            intro_text,
            warning_text,
            confirm_label,
            cancel_label,
        }
    }

    /// Renders the diff dialog. Returns true if "Confirm" is clicked.
    pub fn show(&self, ctx: &egui::Context, open: &mut bool) -> bool {
        let mut confirmed = false;
        if *open {
            egui::Window::new(self.title.as_str())
                .collapsible(false)
                .resizable(true)
                .movable(true)
                .default_size(egui::vec2(780.0, 520.0))
                .show(ctx, |ui| {
                    ui.label(self.intro_text.as_str());
                    ui.separator();

                    ui.heading("Resumen de Cambios");
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("+{}", self.stats.added_events))
                                .color(egui::Color32::GREEN)
                                .strong(),
                        );
                        ui.label("agregados");
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!("~{}", self.stats.modified_events))
                                .color(egui::Color32::YELLOW)
                                .strong(),
                        );
                        ui.label("modificados");
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!("-{}", self.stats.removed_events))
                                .color(egui::Color32::RED)
                                .strong(),
                        );
                        ui.label("eliminados");
                    });

                    if self.previous_script.is_none() {
                        ui.label("Archivo nuevo (sin snapshot previo).");
                    } else {
                        ui.label(format!(
                            "Eventos actuales: {}",
                            self.current_script.events.len()
                        ));
                    }

                    ui.separator();
                    ui.label(
                        egui::RichText::new("Diff horizontal (estilo Git, lado a lado):").strong(),
                    );
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Anterior").monospace().strong());
                        ui.add_space(24.0);
                        ui.label(egui::RichText::new("Actual").monospace().strong());
                    });
                    egui::ScrollArea::both().max_height(300.0).show(ui, |ui| {
                        let full_width = ui.available_width();
                        let marker_width = 18.0;
                        let column_width = ((full_width - marker_width).max(240.0)) / 2.0;
                        egui::Grid::new("diff_horizontal_grid")
                            .striped(true)
                            .num_columns(3)
                            .spacing(egui::vec2(10.0, 2.0))
                            .show(ui, |ui| {
                                for row in &self.lines {
                                    let (marker, color) = match row.kind {
                                        DiffKind::Added => ("+", egui::Color32::GREEN),
                                        DiffKind::Removed => ("-", egui::Color32::RED),
                                        DiffKind::Modified => ("~", egui::Color32::YELLOW),
                                        DiffKind::Context => (" ", egui::Color32::GRAY),
                                        DiffKind::Elided => ("…", egui::Color32::GRAY),
                                    };
                                    let left = render_line_cell(row.left_no, &row.left_text);
                                    let right = render_line_cell(row.right_no, &row.right_text);

                                    ui.add_sized(
                                        [column_width, 0.0],
                                        egui::Label::new(
                                            egui::RichText::new(left).monospace().color(color),
                                        ),
                                    );
                                    ui.add_sized(
                                        [marker_width, 0.0],
                                        egui::Label::new(
                                            egui::RichText::new(marker)
                                                .monospace()
                                                .strong()
                                                .color(color),
                                        ),
                                    );
                                    ui.add_sized(
                                        [column_width, 0.0],
                                        egui::Label::new(
                                            egui::RichText::new(right).monospace().color(color),
                                        ),
                                    );
                                    ui.end_row();
                                }
                            });
                    });

                    ui.separator();
                    ui.label(
                        egui::RichText::new(self.warning_text.as_str())
                            .color(egui::Color32::YELLOW),
                    );

                    ui.horizontal(|ui| {
                        if ui.button(self.cancel_label.as_str()).clicked() {
                            *open = false;
                        }
                        if ui.button(self.confirm_label.as_str()).clicked() {
                            confirmed = true;
                            *open = false;
                        }
                    });
                });
        }
        confirmed
    }
}

fn compute_stats(previous: Option<&ScriptRaw>, current: &ScriptRaw) -> DiffStats {
    let Some(previous) = previous else {
        return DiffStats {
            added_events: current.events.len(),
            removed_events: 0,
            modified_events: 0,
        };
    };

    let mut stats = DiffStats::default();
    let old_len = previous.events.len();
    let new_len = current.events.len();
    let common = old_len.min(new_len);

    for idx in 0..common {
        let old_value = serde_json::to_value(&previous.events[idx]).ok();
        let new_value = serde_json::to_value(&current.events[idx]).ok();
        if old_value != new_value {
            stats.modified_events += 1;
        }
    }

    if new_len > old_len {
        stats.added_events = new_len - old_len;
    } else if old_len > new_len {
        stats.removed_events = old_len - new_len;
    }

    stats
}

fn build_diff_rows(previous: Option<&ScriptRaw>, current: &ScriptRaw) -> Vec<DiffRow> {
    let new_json = current
        .to_json()
        .unwrap_or_else(|_| "<error serializando script actual>".to_string());
    let new_lines: Vec<&str> = new_json.lines().collect();

    let Some(previous) = previous else {
        return new_lines
            .into_iter()
            .enumerate()
            .map(|(idx, line)| DiffRow {
                kind: DiffKind::Added,
                left_no: None,
                right_no: Some(idx),
                left_text: String::new(),
                right_text: line.to_string(),
            })
            .collect();
    };

    let old_json = previous
        .to_json()
        .unwrap_or_else(|_| "<error serializando script previo>".to_string());
    let old_lines: Vec<&str> = old_json.lines().collect();

    if old_lines == new_lines {
        return vec![DiffRow {
            kind: DiffKind::Context,
            left_no: None,
            right_no: None,
            left_text: "Sin cambios de contenido".to_string(),
            right_text: "Sin cambios de contenido".to_string(),
        }];
    }

    let mut prefix = 0usize;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }

    let mut suffix = 0usize;
    while suffix < old_lines.len().saturating_sub(prefix)
        && suffix < new_lines.len().saturating_sub(prefix)
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let mut out = Vec::new();
    push_context_window(&mut out, &old_lines[..prefix], 0, 0, true);

    let old_mid = &old_lines[prefix..old_lines.len().saturating_sub(suffix)];
    let new_mid = &new_lines[prefix..new_lines.len().saturating_sub(suffix)];
    let common_mid = old_mid.len().max(new_mid.len());
    for idx in 0..common_mid {
        let old = old_mid.get(idx).copied();
        let new = new_mid.get(idx).copied();
        let kind = match (old, new) {
            (Some(o), Some(n)) if o == n => DiffKind::Context,
            (Some(_), Some(_)) => DiffKind::Modified,
            (Some(_), None) => DiffKind::Removed,
            (None, Some(_)) => DiffKind::Added,
            (None, None) => DiffKind::Context,
        };
        out.push(DiffRow {
            kind,
            left_no: old.map(|_| prefix + idx),
            right_no: new.map(|_| prefix + idx),
            left_text: old.unwrap_or("").to_string(),
            right_text: new.unwrap_or("").to_string(),
        });
    }

    let old_suffix_start = old_lines.len().saturating_sub(suffix);
    let new_suffix_start = new_lines.len().saturating_sub(suffix);
    push_context_window(
        &mut out,
        &new_lines[new_suffix_start..],
        old_suffix_start,
        new_suffix_start,
        false,
    );

    out
}

fn push_context_window(
    out: &mut Vec<DiffRow>,
    lines: &[&str],
    left_start: usize,
    right_start: usize,
    is_prefix: bool,
) {
    const CONTEXT_LINES: usize = 4;
    if lines.is_empty() {
        return;
    }

    if lines.len() <= CONTEXT_LINES {
        for (idx, line) in lines.iter().enumerate() {
            out.push(DiffRow {
                kind: DiffKind::Context,
                left_no: Some(left_start + idx),
                right_no: Some(right_start + idx),
                left_text: (*line).to_string(),
                right_text: (*line).to_string(),
            });
        }
        return;
    }

    if is_prefix {
        for (idx, line) in lines[..CONTEXT_LINES].iter().enumerate() {
            out.push(DiffRow {
                kind: DiffKind::Context,
                left_no: Some(left_start + idx),
                right_no: Some(right_start + idx),
                left_text: (*line).to_string(),
                right_text: (*line).to_string(),
            });
        }
        out.push(DiffRow {
            kind: DiffKind::Elided,
            left_no: None,
            right_no: None,
            left_text: format!("... {} lineas sin cambios ...", lines.len() - CONTEXT_LINES),
            right_text: String::new(),
        });
    } else {
        out.push(DiffRow {
            kind: DiffKind::Elided,
            left_no: None,
            right_no: None,
            left_text: format!("... {} lineas sin cambios ...", lines.len() - CONTEXT_LINES),
            right_text: String::new(),
        });
        let base = lines.len() - CONTEXT_LINES;
        for (idx, line) in lines[base..].iter().enumerate() {
            let offset = base + idx;
            out.push(DiffRow {
                kind: DiffKind::Context,
                left_no: Some(left_start + offset),
                right_no: Some(right_start + offset),
                left_text: (*line).to_string(),
                right_text: (*line).to_string(),
            });
        }
    }
}

fn render_line_cell(line_no: Option<usize>, text: &str) -> String {
    let rendered = text.replace('\t', "    ");
    match line_no {
        Some(no) => format!("{:>4} {}", no + 1, rendered),
        None => format!("     {}", rendered),
    }
}
