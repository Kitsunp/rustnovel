use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

#[derive(Default)]
struct LineStats {
    files: usize,
    code: usize,
    comments: usize,
    blanks: usize,
}

#[derive(Clone, Copy)]
enum CommentStyle {
    Slash,
    Hash,
    Html,
    None,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse()?;
    let root = env::current_dir()?;
    let report = build_report(&root)?;

    match args.output {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, report)?;
            println!("Reporte de lineas generado en {}", path.display());
        }
        None => {
            print!("{report}");
        }
    }

    Ok(())
}

fn build_report(root: &Path) -> Result<String, Box<dyn Error>> {
    let mut by_language: BTreeMap<String, LineStats> = BTreeMap::new();

    for entry in WalkDir::new(root).into_iter().filter_entry(should_descend) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let source = match fs::read_to_string(entry.path()) {
            Ok(source) => source,
            Err(_) => continue,
        };

        let (language, style) = detect_language(entry.path());
        let (code, comments, blanks) = count_lines(&source, style);
        if code + comments + blanks == 0 {
            continue;
        }

        let row = by_language.entry(language.to_string()).or_default();
        row.files += 1;
        row.code += code;
        row.comments += comments;
        row.blanks += blanks;
    }

    let mut rows: Vec<(String, LineStats)> = by_language.into_iter().collect();
    rows.sort_by(|left, right| {
        right
            .1
            .code
            .cmp(&left.1.code)
            .then_with(|| left.0.cmp(&right.0))
    });

    let total_files: usize = rows.iter().map(|(_, row)| row.files).sum();
    let total_code: usize = rows.iter().map(|(_, row)| row.code).sum();
    let total_comments: usize = rows.iter().map(|(_, row)| row.comments).sum();
    let total_blanks: usize = rows.iter().map(|(_, row)| row.blanks).sum();

    let mut report = String::new();
    writeln!(&mut report, "# Reporte de lineas del repositorio")?;
    writeln!(&mut report)?;
    writeln!(&mut report, "Ruta analizada: `{}`", root.display())?;
    writeln!(&mut report)?;
    writeln!(&mut report, "## Desglose por lenguaje")?;
    writeln!(&mut report)?;
    writeln!(
        &mut report,
        "| Lenguaje | Archivos | Codigo | Comentarios | Blancos |"
    )?;
    writeln!(&mut report, "| --- | ---: | ---: | ---: | ---: |")?;
    for (language, row) in rows {
        writeln!(
            &mut report,
            "| {} | {} | {} | {} | {} |",
            language, row.files, row.code, row.comments, row.blanks
        )?;
    }
    writeln!(&mut report)?;
    writeln!(&mut report, "## Totales")?;
    writeln!(&mut report)?;
    writeln!(&mut report, "- Archivos: {total_files}")?;
    writeln!(&mut report, "- Codigo: {total_code}")?;
    writeln!(&mut report, "- Comentarios: {total_comments}")?;
    writeln!(&mut report, "- Blancos: {total_blanks}")?;

    Ok(report)
}

fn should_descend(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }

    let name = entry.file_name().to_string_lossy();
    name != ".git" && name != "target"
}

fn detect_language(path: &Path) -> (&'static str, CommentStyle) {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return ("Otros", CommentStyle::None);
    };

    match ext.to_ascii_lowercase().as_str() {
        "rs" => ("Rust", CommentStyle::Slash),
        "py" => ("Python", CommentStyle::Hash),
        "toml" => ("TOML", CommentStyle::Hash),
        "yaml" | "yml" => ("YAML", CommentStyle::Hash),
        "json" => ("JSON", CommentStyle::None),
        "md" => ("Markdown", CommentStyle::None),
        "sh" | "bash" | "zsh" | "ps1" | "psm1" => ("Scripts", CommentStyle::Hash),
        "js" | "ts" | "tsx" | "jsx" | "c" | "h" | "cpp" | "hpp" | "java" | "kt" | "swift"
        | "go" | "css" => ("C-like", CommentStyle::Slash),
        "html" | "xml" | "svg" => ("Markup", CommentStyle::Html),
        _ => ("Otros", CommentStyle::None),
    }
}

fn count_lines(source: &str, style: CommentStyle) -> (usize, usize, usize) {
    let mut code = 0usize;
    let mut comments = 0usize;
    let mut blanks = 0usize;
    let mut in_block_comment = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blanks += 1;
            continue;
        }

        match style {
            CommentStyle::Slash => {
                if in_block_comment {
                    comments += 1;
                    if trimmed.contains("*/") {
                        in_block_comment = false;
                    }
                    continue;
                }

                if trimmed.starts_with("//") {
                    comments += 1;
                } else if trimmed.starts_with("/*") {
                    comments += 1;
                    if !trimmed.contains("*/") {
                        in_block_comment = true;
                    }
                } else {
                    code += 1;
                }
            }
            CommentStyle::Hash => {
                if trimmed.starts_with('#') {
                    comments += 1;
                } else {
                    code += 1;
                }
            }
            CommentStyle::Html => {
                if in_block_comment {
                    comments += 1;
                    if trimmed.contains("-->") {
                        in_block_comment = false;
                    }
                    continue;
                }

                if trimmed.starts_with("<!--") {
                    comments += 1;
                    if !trimmed.contains("-->") {
                        in_block_comment = true;
                    }
                } else {
                    code += 1;
                }
            }
            CommentStyle::None => {
                code += 1;
            }
        }
    }

    (code, comments, blanks)
}

struct Args {
    output: Option<PathBuf>,
}

impl Args {
    fn parse() -> Result<Self, Box<dyn Error>> {
        let mut output = None;
        let mut args = env::args().skip(1);

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-o" | "--output" => {
                    let path = args
                        .next()
                        .ok_or("Falta el argumento de ruta para --output")?;
                    output = Some(PathBuf::from(path));
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => {
                    return Err(format!("Argumento desconocido: {arg}").into());
                }
            }
        }

        Ok(Self { output })
    }
}

fn print_help() {
    println!(
        "Uso: cargo run --bin repo_report -- [--output <ruta>]\n\
\n\
Opciones:\n\
  -o, --output <ruta>  Escribe el reporte en una ruta especifica\n\
  -h, --help           Muestra esta ayuda\n"
    );
}
