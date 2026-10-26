#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! Visual Novel Editor - A visual authoring tool for visual novels.
//!
//! This binary launches the editor workbench for creating and editing
//! visual novel scripts with timeline, graph, and viewport panels.

fn main() {
    if let Err(err) = tracing_subscriber::fmt::try_init() {
        eprintln!("Editor logging already initialized or unavailable: {err}");
    }
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("Usage: vn_editor [project.vnm]");
        return;
    }
    let interactive = args.is_empty() || (args.len() == 1 && !args[0].starts_with('-'));
    let initial_project = match parse_initial_project(args) {
        Ok(value) => value,
        Err(err) => {
            visual_novel_gui::report_startup_error(
                &format!("Error running editor: {err}"),
                interactive,
            );
            std::process::exit(1);
        }
    };
    match visual_novel_gui::run_editor_with_project(initial_project) {
        Ok(()) => {}
        Err(e) => {
            visual_novel_gui::report_startup_error(
                &format!("Error running editor: {e}"),
                interactive,
            );
            std::process::exit(1);
        }
    }
}

fn parse_initial_project(
    args: impl IntoIterator<Item = String>,
) -> Result<Option<std::path::PathBuf>, String> {
    let mut project = None;
    for arg in args {
        if arg.starts_with('-') {
            return Err(format!("unknown editor argument '{arg}'"));
        }
        if project.replace(std::path::PathBuf::from(arg)).is_some() {
            return Err("editor accepts only one project manifest path".to_string());
        }
    }
    Ok(project)
}
