#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! Visual Novel Editor - A visual authoring tool for visual novels.
//!
//! This binary launches the editor workbench for creating and editing
//! visual novel scripts with timeline, graph, and viewport panels.

fn main() {
    if let Err(err) = tracing_subscriber::fmt::try_init() {
        eprintln!("Editor logging already initialized or unavailable: {err}");
    }
    let initial_project = match parse_initial_project(std::env::args().skip(1)) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Error running editor: {err}");
            std::process::exit(1);
        }
    };
    match visual_novel_gui::run_editor_with_project(initial_project) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error running editor: {e}");
            std::process::exit(1);
        }
    }
}

fn parse_initial_project(
    args: impl IntoIterator<Item = String>,
) -> Result<Option<std::path::PathBuf>, String> {
    let mut project = None;
    for arg in args {
        if arg == "--help" || arg == "-h" {
            println!("Usage: vn_editor [project.vnm]");
            return Ok(None);
        }
        if arg.starts_with('-') {
            return Err(format!("unknown editor argument '{arg}'"));
        }
        if project.replace(std::path::PathBuf::from(arg)).is_some() {
            return Err("editor accepts only one project manifest path".to_string());
        }
    }
    Ok(project)
}
